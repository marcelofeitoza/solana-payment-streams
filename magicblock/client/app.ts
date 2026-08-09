//! High-level operations shared by every CLI command: local-stack bootstrap, mint/token setup,
//! the one sender-signed init+delegate+fund transaction, crank scheduling, and settlement polling.

import {
  Connection,
  PublicKey,
  Signer,
  SystemProgram,
  Transaction,
  TransactionInstruction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import {
  createAssociatedTokenAccountIdempotentInstruction,
  createInitializeMint2Instruction,
  createMintToCheckedInstruction,
  getAssociatedTokenAddressSync,
  getMinimumBalanceForRentExemptMint,
  MINT_SIZE,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";

import {
  DLP_PROGRAM_ID,
  crankSignerPda,
  delegationRecordPda,
  escrowAuthorityPda,
  protocolFeeVaultPda,
  streamPda,
  validatorFeeVaultPda,
} from "./addresses.js";
import { initProtocolFeesVault, initValidatorFeesVault, localValidatorKeypair, topUpEphemeralBalance } from "./dlp.js";
import {
  LocalConfig,
  loadConfig,
  loadOrCreateKeypair,
  mintPath,
  operatorPath,
  pk,
  recipientPath,
  saveConfig,
  senderPath,
} from "./state.js";
import { delegateStream, initializeStream, scheduleStream } from "./wire.js";

const USDC_DECIMALS = 6;
const USDC_SCALE = 1_000_000n;
const DEFAULT_FEE_RESERVE_LAMPORTS = 5_000_000_000n;
const SENDER_REQUIRED_LAMPORTS = 20_000_000_000;
const OPERATOR_REQUIRED_LAMPORTS = 5_000_000_000;
const LOCAL_VALIDATOR_REQUIRED_LAMPORTS = 100_000_000_000;

export function wholeUsdcToBaseUnits(whole: bigint): bigint {
  return whole * USDC_SCALE;
}

export function formatUsdc(baseUnits: bigint): string {
  const whole = baseUnits / USDC_SCALE;
  const fractional = baseUnits % USDC_SCALE;
  return `${whole}.${fractional.toString().padStart(6, "0")} USDC`;
}

function ceilDiv(value: bigint, divisor: bigint): bigint {
  if (divisor === 0n) throw new Error("division by zero");
  return (value + divisor - 1n) / divisor;
}

async function send(
  connection: Connection,
  payer: PublicKey,
  instructions: TransactionInstruction[],
  signers: Signer[],
): Promise<string> {
  const transaction = new Transaction().add(...instructions);
  transaction.feePayer = payer;
  return sendAndConfirmTransaction(connection, transaction, signers, { commitment: "confirmed" });
}

async function ensureFunded(connection: Connection, address: PublicKey, minimum: number) {
  const balance = await connection.getBalance(address, "confirmed");
  if (balance >= minimum) return;
  const signature = await connection.requestAirdrop(address, minimum - balance);
  await connection.confirmTransaction(signature, "confirmed");
}

async function waitForAccount(connection: Connection, address: PublicKey, timeoutMs: number) {
  const started = Date.now();
  while (true) {
    if ((await connection.getAccountInfo(address, "confirmed")) !== null) return;
    if (Date.now() - started >= timeoutMs) {
      throw new Error(`account ${address.toBase58()} did not appear before timeout`);
    }
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
}

/// Classic SPL Token account layout: amount is an 8-byte LE u64 at offset 64. Missing account = 0.
async function tokenAmount(connection: Connection, address: PublicKey): Promise<bigint> {
  const account = await connection.getAccountInfo(address, "confirmed");
  if (!account) return 0n;
  return account.data.readBigUInt64LE(64);
}

/// Initialize the local DLP protocol and validator fee vaults. Idempotent; a prerequisite for
/// starting the local Ephemeral Rollup validator (it queries these on boot).
export async function bootstrapLocalDlp(base: Connection): Promise<string[]> {
  await waitForAccount(base, DLP_PROGRAM_ID, 30_000);
  const dlp = await base.getAccountInfo(DLP_PROGRAM_ID, "confirmed");
  if (!dlp?.executable) throw new Error("local DLP account is not executable");

  const validator = localValidatorKeypair();
  await ensureFunded(base, validator.publicKey, LOCAL_VALIDATOR_REQUIRED_LAMPORTS);

  const signatures: string[] = [];
  if (!(await base.getAccountInfo(protocolFeeVaultPda(), "confirmed"))) {
    signatures.push(await send(base, validator.publicKey, [initProtocolFeesVault(validator.publicKey)], [validator]));
  }
  if (!(await base.getAccountInfo(validatorFeeVaultPda(validator.publicKey), "confirmed"))) {
    signatures.push(
      await send(
        base,
        validator.publicKey,
        [initValidatorFeesVault(validator.publicKey, validator.publicKey, validator.publicKey)],
        [validator],
      ),
    );
  }
  return signatures;
}

/// Create the local six-decimal mint and all sender/recipient/escrow token accounts.
export async function initUsdc(base: Connection, stateDir: string, validator: PublicKey): Promise<LocalConfig> {
  const existing = await loadConfig(stateDir).catch(() => null);
  if (existing && (await base.getAccountInfo(pk(existing.mint), "confirmed"))) {
    const sender = await loadOrCreateKeypair(senderPath(stateDir));
    const operator = await loadOrCreateKeypair(operatorPath(stateDir));
    await ensureFunded(base, sender.publicKey, SENDER_REQUIRED_LAMPORTS);
    await ensureFunded(base, operator.publicKey, OPERATOR_REQUIRED_LAMPORTS);
    const mint = pk(existing.mint);
    await send(
      base,
      sender.publicKey,
      [
        createAssociatedTokenAccountIdempotentInstruction(sender.publicKey, pk(existing.senderTokenAccount), sender.publicKey, mint),
        createAssociatedTokenAccountIdempotentInstruction(sender.publicKey, pk(existing.recipientTokenAccount), pk(existing.recipient), mint),
        createAssociatedTokenAccountIdempotentInstruction(sender.publicKey, pk(existing.escrowTokenAccount), pk(existing.escrowAuthority), mint),
      ],
      [sender],
    );
    return existing;
  }

  const sender = await loadOrCreateKeypair(senderPath(stateDir));
  const recipient = await loadOrCreateKeypair(recipientPath(stateDir));
  const operator = await loadOrCreateKeypair(operatorPath(stateDir));
  const mint = await loadOrCreateKeypair(mintPath(stateDir));

  await ensureFunded(base, sender.publicKey, SENDER_REQUIRED_LAMPORTS);
  await ensureFunded(base, operator.publicKey, OPERATOR_REQUIRED_LAMPORTS);

  const [stream, streamBump] = streamPda(sender.publicKey, recipient.publicKey, mint.publicKey);
  const [escrowAuthority, escrowBump] = escrowAuthorityPda(stream);
  const senderToken = getAssociatedTokenAddressSync(mint.publicKey, sender.publicKey);
  const recipientToken = getAssociatedTokenAddressSync(mint.publicKey, recipient.publicKey);
  const escrowToken = getAssociatedTokenAddressSync(mint.publicKey, escrowAuthority, true);

  const mintRent = await getMinimumBalanceForRentExemptMint(base);
  await send(
    base,
    sender.publicKey,
    [
      SystemProgram.createAccount({
        fromPubkey: sender.publicKey,
        newAccountPubkey: mint.publicKey,
        lamports: mintRent,
        space: MINT_SIZE,
        programId: TOKEN_PROGRAM_ID,
      }),
      createInitializeMint2Instruction(mint.publicKey, USDC_DECIMALS, sender.publicKey, null),
      createAssociatedTokenAccountIdempotentInstruction(sender.publicKey, senderToken, sender.publicKey, mint.publicKey),
      createAssociatedTokenAccountIdempotentInstruction(sender.publicKey, recipientToken, recipient.publicKey, mint.publicKey),
      createAssociatedTokenAccountIdempotentInstruction(sender.publicKey, escrowToken, escrowAuthority, mint.publicKey),
    ],
    [sender, mint],
  );

  const config: LocalConfig = {
    sender: sender.publicKey.toBase58(),
    recipient: recipient.publicKey.toBase58(),
    operator: operator.publicKey.toBase58(),
    mint: mint.publicKey.toBase58(),
    senderTokenAccount: senderToken.toBase58(),
    recipientTokenAccount: recipientToken.toBase58(),
    stream: stream.toBase58(),
    escrowAuthority: escrowAuthority.toBase58(),
    escrowTokenAccount: escrowToken.toBase58(),
    validator: validator.toBase58(),
    streamBump,
    escrowBump,
  };
  await saveConfig(stateDir, config);
  return config;
}

/// Mint an integer number of whole USDC to the sender account.
export async function mint(base: Connection, stateDir: string, wholeUsdc: bigint): Promise<{ signature: string; balance: bigint }> {
  const config = await loadConfig(stateDir);
  const sender = await loadOrCreateKeypair(senderPath(stateDir));
  const amount = wholeUsdcToBaseUnits(wholeUsdc);
  const instruction = createMintToCheckedInstruction(
    pk(config.mint),
    pk(config.senderTokenAccount),
    sender.publicKey,
    amount,
    USDC_DECIMALS,
  );
  const signature = await send(base, sender.publicKey, [instruction], [sender]);
  const balance = await tokenAmount(base, pk(config.senderTokenAccount));
  return { signature, balance };
}

export type InitStreamResult = { signature: string; iterations: bigint; totalAmount: bigint; chunkAmount: bigint };

/// One sender-signed transaction escrows all funds, initializes state, funds the base-layer
/// action escrow, and delegates the stream to the Ephemeral Rollup.
export async function initStream(
  base: Connection,
  stateDir: string,
  totalWholeUsdc: bigint,
  chunkWholeUsdc: bigint,
  intervalMs: bigint,
  feeReserveLamports: bigint,
): Promise<InitStreamResult> {
  const config = await loadConfig(stateDir);
  const sender = await loadOrCreateKeypair(senderPath(stateDir));
  const totalAmount = wholeUsdcToBaseUnits(totalWholeUsdc);
  const chunkAmount = wholeUsdcToBaseUnits(chunkWholeUsdc);
  if (totalAmount <= 0n || chunkAmount <= 0n || chunkAmount > totalAmount || intervalMs <= 0n) {
    throw new Error("total, chunk, and interval must be positive; chunk must not exceed total");
  }
  if (feeReserveLamports < DEFAULT_FEE_RESERVE_LAMPORTS) {
    throw new Error(`fee reserve must be at least ${DEFAULT_FEE_RESERVE_LAMPORTS} lamports`);
  }
  const initialSender = await tokenAmount(base, pk(config.senderTokenAccount));
  const initialRecipient = await tokenAmount(base, pk(config.recipientTokenAccount));
  if (initialSender < totalAmount) {
    throw new Error(`sender has ${initialSender} base units but stream needs ${totalAmount}`);
  }

  const stream = pk(config.stream);
  const initialize = initializeStream(
    sender.publicKey,
    pk(config.recipient),
    pk(config.mint),
    pk(config.senderTokenAccount),
    pk(config.recipientTokenAccount),
    pk(config.escrowTokenAccount),
    pk(config.escrowAuthority),
    stream,
    pk(config.validator),
    totalAmount,
    chunkAmount,
    intervalMs,
    feeReserveLamports,
    config.streamBump,
    config.escrowBump,
  );
  // A CallHandler executes on the base layer through the DLP's actor escrow. It is
  // deliberately distinct from both the delegated stream fee balance and the SPL token
  // escrow authority. Funding it here keeps the complete authorization flow in this one
  // sender-signed transaction.
  const topUpActionEscrow = topUpEphemeralBalance(sender.publicKey, stream, feeReserveLamports, 0);
  const delegate = delegateStream(sender.publicKey, stream, pk(config.validator));

  const signature = await send(base, sender.publicKey, [initialize, topUpActionEscrow, delegate], [sender]);

  await saveConfig(stateDir, {
    ...config,
    totalAmount: totalAmount.toString(),
    chunkAmount: chunkAmount.toString(),
    intervalMs: intervalMs.toString(),
    initialSenderAmount: initialSender.toString(),
    initialRecipientAmount: initialRecipient.toString(),
  });
  return { signature, iterations: ceilDiv(totalAmount, chunkAmount), totalAmount, chunkAmount };
}

/// Relayer-paid scheduling. The Pinocchio program PDA-signs the Native Crank CPI.
export async function schedule(base: Connection, ephemeral: Connection, stateDir: string): Promise<string> {
  const config = await loadConfig(stateDir);
  const operator = await loadOrCreateKeypair(operatorPath(stateDir));
  const stream = pk(config.stream);
  await waitForAccount(ephemeral, stream, 30_000);
  await waitForAccount(ephemeral, delegationRecordPda(stream), 30_000);
  const instruction = scheduleStream(stream, crankSignerPda(stream), pk(config.escrowAuthority), pk(config.validator));
  return send(ephemeral, operator.publicKey, [instruction], [operator]);
}

export type BalanceSnapshot = { sender: bigint; escrow: bigint; recipient: bigint };

export function printBalances(snapshot: BalanceSnapshot) {
  console.log(`sender=${snapshot.sender} base_units (${formatUsdc(snapshot.sender)})`);
  console.log(`escrow=${snapshot.escrow} base_units (${formatUsdc(snapshot.escrow)})`);
  console.log(`recipient=${snapshot.recipient} base_units (${formatUsdc(snapshot.recipient)})`);
}

export async function balances(base: Connection, stateDir: string): Promise<BalanceSnapshot> {
  const config = await loadConfig(stateDir);
  return {
    sender: await tokenAmount(base, pk(config.senderTokenAccount)),
    escrow: await tokenAmount(base, pk(config.escrowTokenAccount)),
    recipient: await tokenAmount(base, pk(config.recipientTokenAccount)),
  };
}

function executionsForReceived(received: bigint, total: bigint, chunk: bigint): bigint {
  if (received === total) return ceilDiv(total, chunk);
  if (received % chunk !== 0n) throw new Error(`recipient delta ${received} is not an exact chunk multiple`);
  return received / chunk;
}

export type WatchReport = { executions: bigint; expectedExecutions: bigint; finalBalances: BalanceSnapshot };

/// Monitor base-layer settlement and print one exact line for every inferred execution.
export async function watch(base: Connection, stateDir: string, timeoutSeconds: number): Promise<WatchReport> {
  const config = await loadConfig(stateDir);
  if (!config.totalAmount || !config.chunkAmount || !config.initialSenderAmount || !config.initialRecipientAmount) {
    throw new Error("stream is not initialized");
  }
  const total = BigInt(config.totalAmount);
  const chunk = BigInt(config.chunkAmount);
  const initialSender = BigInt(config.initialSenderAmount);
  const initialRecipient = BigInt(config.initialRecipientAmount);
  const expectedExecutions = ceilDiv(total, chunk);
  const sourceAfterEscrow = initialSender - total;
  if (sourceAfterEscrow < 0n) throw new Error("initial sender balance is below total");

  const started = Date.now();
  let printedExecutions = 0n;
  while (true) {
    const recipientBalance = await tokenAmount(base, pk(config.recipientTokenAccount));
    const received = recipientBalance - initialRecipient;
    if (received < 0n) throw new Error("recipient balance fell below its initial value");
    if (received > total) throw new Error(`recipient received ${received}, more than stream total ${total}`);
    const observedExecutions = executionsForReceived(received, total, chunk);
    while (printedExecutions < observedExecutions) {
      printedExecutions += 1n;
      const cumulative = printedExecutions * chunk < total ? printedExecutions * chunk : total;
      console.log(
        `execution=${printedExecutions}/${expectedExecutions} sender=${sourceAfterEscrow} escrow=${total - cumulative} recipient=${initialRecipient + cumulative}`,
      );
    }
    if (received === total) {
      return { executions: printedExecutions, expectedExecutions, finalBalances: await balances(base, stateDir) };
    }
    if ((Date.now() - started) / 1000 >= timeoutSeconds) {
      throw new Error(`watch timed out after ${timeoutSeconds} seconds at execution ${printedExecutions}/${expectedExecutions}`);
    }
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
}
