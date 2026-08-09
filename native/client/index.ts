import { Connection, PublicKey, Transaction, sendAndConfirmTransaction } from "@solana/web3.js";
import {
  createAssociatedTokenAccountIdempotentInstruction,
  createMint,
  getAssociatedTokenAddressSync,
  mintTo,
} from "@solana/spl-token";

import { loadOrCreate, loadState, saveState } from "./state.js";
import { initializeInstruction, releaseInstruction, streamPda } from "./wire.js";

const RPC_URL = process.env.RPC_URL ?? "http://127.0.0.1:9910";
const TOTAL = BigInt(process.env.TOTAL_BASE_UNITS ?? "1000000000000");
const CHUNK = BigInt(process.env.CHUNK_BASE_UNITS ?? "100000000");

/// Retries a fresh transaction (new blockhash each attempt) on transient send/simulation
/// failures. Safe to retry blindly: the program's own release logic treats a stale or
/// already-applied chunk index as a successful no-op, so a retry can never double-spend.
async function sendWithRetry(
  connection: Connection,
  build: () => Transaction,
  signers: Parameters<typeof sendAndConfirmTransaction>[2],
  attempts = 8,
): Promise<string> {
  let lastError: unknown;
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    try {
      return await sendAndConfirmTransaction(connection, build(), signers, { commitment: "confirmed" });
    } catch (error) {
      lastError = error;
      await new Promise((resolve) => setTimeout(resolve, 50 * (attempt + 1)));
    }
  }
  throw lastError;
}

async function setup(connection: Connection) {
  const bootstrap = await loadOrCreate(".local/bootstrap.json");
  const sender = await loadOrCreate(".local/sender.json");
  const recipient = await loadOrCreate(".local/recipient.json");
  const keeper = await loadOrCreate(".local/keeper.json");
  for (const account of [bootstrap, sender, keeper]) {
    const signature = await connection.requestAirdrop(account.publicKey, 5_000_000_000);
    await connection.confirmTransaction(signature, "confirmed");
  }
  const mint = await createMint(connection, bootstrap, bootstrap.publicKey, null, 6);
  const sourceToken = getAssociatedTokenAddressSync(mint, sender.publicKey);
  const sourceInstruction = createAssociatedTokenAccountIdempotentInstruction(
    bootstrap.publicKey, sourceToken, sender.publicKey, mint,
  );
  await sendAndConfirmTransaction(connection, new Transaction().add(sourceInstruction), [bootstrap]);
  await mintTo(connection, bootstrap, mint, sourceToken, bootstrap, TOTAL);
  await saveState({
    bootstrap: bootstrap.publicKey.toBase58(), sender: sender.publicKey.toBase58(),
    recipient: recipient.publicKey.toBase58(), keeper: keeper.publicKey.toBase58(),
    mint: mint.toBase58(), sourceToken: sourceToken.toBase58(),
  });
  console.log(`mint=${mint.toBase58()} sender=${sender.publicKey.toBase58()}`);
}

async function initialize(connection: Connection) {
  const state = await loadState();
  const sender = await loadOrCreate(".local/sender.json");
  const recipient = new PublicKey(state.recipient);
  const mint = new PublicKey(state.mint);
  const [stream, bump] = streamPda(sender.publicKey, recipient, mint, 1n);
  const vault = getAssociatedTokenAddressSync(mint, stream, true);
  const recipientToken = getAssociatedTokenAddressSync(mint, recipient);
  const transaction = new Transaction().add(
    createAssociatedTokenAccountIdempotentInstruction(sender.publicKey, recipientToken, recipient, mint),
    createAssociatedTokenAccountIdempotentInstruction(sender.publicKey, vault, stream, mint),
    initializeInstruction(state, stream, vault, recipientToken, bump, TOTAL, CHUNK),
  );
  const signature = await sendAndConfirmTransaction(connection, transaction, [sender]);
  await saveState({ ...state, stream: stream.toBase58(), vault: vault.toBase58(), recipientToken: recipientToken.toBase58() });
  console.log(`initialize=${signature} stream=${stream.toBase58()}`);
}

async function run(connection: Connection) {
  const state = await loadState();
  if (!state.stream || !state.vault || !state.recipientToken) throw new Error("stream is not initialized");
  const keeper = await loadOrCreate(".local/keeper.json");
  const stream = new PublicKey(state.stream);
  const vault = new PublicKey(state.vault);
  const recipientToken = new PublicKey(state.recipientToken);
  const mint = new PublicKey(state.mint);
  while (true) {
    const account = await connection.getAccountInfo(stream);
    if (!account) throw new Error("stream account is closed");
    if (account.data[9] !== 1) break; // status != Active
    const index = account.data.readBigUInt64LE(240); // executed_chunks
    const maxChunks = account.data.readBigUInt64LE(248); // max_chunks
    const signature = await sendWithRetry(
      connection,
      () => new Transaction().add(releaseInstruction(keeper.publicKey, stream, vault, recipientToken, mint, index)),
      [keeper],
    );
    console.log(`execution=${index + 1n}/${maxChunks} tx=${signature}`);
  }
}

async function main() {
  const connection = new Connection(RPC_URL, "confirmed");
  const action = process.argv[2] ?? "demo";
  if (action === "setup") return setup(connection);
  if (action === "init-stream") return initialize(connection);
  if (action === "run") return run(connection);
  if (action === "demo") {
    await setup(connection);
    await initialize(connection);
    return run(connection);
  }
  throw new Error(`unknown action: ${action}`);
}

await main();
