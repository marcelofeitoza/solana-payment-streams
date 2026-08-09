//! Native USDC stream wire format: PDA seeds and instruction encoding.
//! Must stay byte-for-byte in sync with native/program/src/{constants,instructions}.rs.

import { AccountMeta, PublicKey, SystemProgram, TransactionInstruction } from "@solana/web3.js";
import type { LocalState } from "./state.js";

export const PROGRAM_ID = new PublicKey("2iGXTHjaBJW6auyKm7V3ZcbBcQMfxHJfVoucx8XhCH6V");
export const CLOCK_SYSVAR = new PublicKey("SysvarC1ock11111111111111111111111111111111");
export const TOKEN_PROGRAM_ID = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

export function u64(value: bigint): Buffer {
  const result = Buffer.alloc(8);
  result.writeBigUInt64LE(value);
  return result;
}

export function i64(value: bigint): Buffer {
  const result = Buffer.alloc(8);
  result.writeBigInt64LE(value);
  return result;
}

/// Seeds: ["stream", sender, recipient, mint, stream_id (LE)].
export function streamPda(sender: PublicKey, recipient: PublicKey, mint: PublicKey, id: bigint) {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("stream"), sender.toBytes(), recipient.toBytes(), mint.toBytes(), u64(id)],
    PROGRAM_ID,
  );
}

/// Tag 0. Data: [stream_id: 8][total_amount: 8][chunk_amount: 8][start_timestamp: 8][interval_seconds: 8][stream_bump: 1].
export function initializeData(total: bigint, chunk: bigint, bump: number): Buffer {
  return Buffer.concat([Buffer.from([0]), u64(1n), u64(total), u64(chunk), i64(0n), i64(0n), Buffer.from([bump])]);
}

/// Tag 1. Data: [expected_chunk_index: 8].
export function releaseData(index: bigint): Buffer {
  return Buffer.concat([Buffer.from([1]), u64(index)]);
}

export function initializeInstruction(
  state: LocalState,
  stream: PublicKey,
  vault: PublicKey,
  recipientToken: PublicKey,
  bump: number,
  total: bigint,
  chunk: bigint,
): TransactionInstruction {
  const accounts: AccountMeta[] = [
    { pubkey: new PublicKey(state.sender), isSigner: true, isWritable: true },
    { pubkey: new PublicKey(state.recipient), isSigner: false, isWritable: false },
    { pubkey: new PublicKey(state.mint), isSigner: false, isWritable: false },
    { pubkey: new PublicKey(state.sourceToken), isSigner: false, isWritable: true },
    { pubkey: vault, isSigner: false, isWritable: true },
    { pubkey: recipientToken, isSigner: false, isWritable: true },
    { pubkey: stream, isSigner: false, isWritable: true },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    { pubkey: CLOCK_SYSVAR, isSigner: false, isWritable: false },
  ];
  return new TransactionInstruction({ programId: PROGRAM_ID, keys: accounts, data: initializeData(total, chunk, bump) });
}

export function releaseInstruction(
  keeper: PublicKey,
  stream: PublicKey,
  vault: PublicKey,
  recipientToken: PublicKey,
  mint: PublicKey,
  index: bigint,
): TransactionInstruction {
  const accounts: AccountMeta[] = [
    { pubkey: keeper, isSigner: true, isWritable: false },
    { pubkey: stream, isSigner: false, isWritable: true },
    { pubkey: vault, isSigner: false, isWritable: true },
    { pubkey: recipientToken, isSigner: false, isWritable: true },
    { pubkey: mint, isSigner: false, isWritable: false },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: CLOCK_SYSVAR, isSigner: false, isWritable: false },
  ];
  return new TransactionInstruction({ programId: PROGRAM_ID, keys: accounts, data: releaseData(index) });
}
