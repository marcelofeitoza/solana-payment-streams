//! MagicBlock Delegation Program (DLP) instructions needed to bootstrap and fund one local
//! stream. This client never builds SettleChunk/Finalize/UndelegateCallback: those base-layer
//! instructions are invoked automatically by the DLP itself when a scheduled Magic Action runs.

import bs58 from "bs58";
import { AccountMeta, Keypair, PublicKey, SystemProgram, TransactionInstruction } from "@solana/web3.js";

import {
  DLP_PROGRAM_ID,
  delegationProgramDataPda,
  ephemeralBalancePda,
  protocolFeeVaultPda,
  validatorFeeVaultPda,
} from "./addresses.js";

/// Official MagicBlock local development key. Never use this public test key in production.
const LOCAL_VALIDATOR_KEYPAIR_BASE58 =
  "9Vo7TbA5YfC5a33JhAi9Fb41usA6JwecHNRw3f9MzzHAM8hFnXTzL5DcEHwsAFjuUZ8vNQcJ4XziRFpMc3gTgBQ";

export function localValidatorKeypair(): Keypair {
  return Keypair.fromSecretKey(bs58.decode(LOCAL_VALIDATOR_KEYPAIR_BASE58));
}

function discriminator(value: number): Buffer {
  const buffer = Buffer.alloc(8);
  buffer.writeBigUInt64LE(BigInt(value));
  return buffer;
}

/// DLP tag 5: create the one protocol-wide fee vault (idempotent prerequisite, run once).
export function initProtocolFeesVault(payer: PublicKey): TransactionInstruction {
  const accounts: AccountMeta[] = [
    { pubkey: payer, isSigner: true, isWritable: true },
    { pubkey: protocolFeeVaultPda(), isSigner: false, isWritable: true },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
  ];
  return new TransactionInstruction({ programId: DLP_PROGRAM_ID, keys: accounts, data: discriminator(5) });
}

/// DLP tag 6: create one validator's fee vault (idempotent prerequisite, run once per validator).
export function initValidatorFeesVault(
  payer: PublicKey,
  admin: PublicKey,
  validatorIdentity: PublicKey,
): TransactionInstruction {
  const accounts: AccountMeta[] = [
    { pubkey: payer, isSigner: true, isWritable: true },
    { pubkey: admin, isSigner: true, isWritable: true },
    { pubkey: delegationProgramDataPda(), isSigner: false, isWritable: false },
    { pubkey: validatorIdentity, isSigner: false, isWritable: true },
    { pubkey: validatorFeeVaultPda(validatorIdentity), isSigner: false, isWritable: true },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
  ];
  return new TransactionInstruction({ programId: DLP_PROGRAM_ID, keys: accounts, data: discriminator(6) });
}

/// DLP tag 9: fund the base-layer escrow that pays for one stream's scheduled Magic Actions.
/// Bundled into the same sender-signed transaction as Initialize + Delegate.
export function topUpEphemeralBalance(
  payer: PublicKey,
  pubkey: PublicKey,
  amount: bigint,
  index: number,
): TransactionInstruction {
  const args = Buffer.alloc(9);
  args.writeBigUInt64LE(amount, 0);
  args.writeUInt8(index, 8);
  const accounts: AccountMeta[] = [
    { pubkey: payer, isSigner: true, isWritable: true },
    { pubkey, isSigner: false, isWritable: false },
    { pubkey: ephemeralBalancePda(pubkey, index), isSigner: false, isWritable: true },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
  ];
  return new TransactionInstruction({
    programId: DLP_PROGRAM_ID,
    keys: accounts,
    data: Buffer.concat([discriminator(9), args]),
  });
}
