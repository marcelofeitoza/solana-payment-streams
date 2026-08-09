//! MagicBlock USDC stream program instructions this client builds directly: Initialize,
//! Delegate, Schedule. Release/SettleChunk/Finalize/UndelegateCallback are invoked
//! automatically by the Native Crank and the DLP once a stream is scheduled.
//! Must stay byte-for-byte in sync with magicblock/program/src/{constants,instructions}.rs.

import { AccountMeta, PublicKey, SystemProgram, TransactionInstruction } from "@solana/web3.js";

import {
  DLP_PROGRAM_ID,
  MAGIC_CONTEXT_ID,
  MAGIC_PROGRAM_ID,
  PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  delegationBufferPda,
  delegationMetadataPda,
  delegationRecordPda,
  magicFeeVaultPda,
} from "./addresses.js";

function discriminator(value: number): Buffer {
  const buffer = Buffer.alloc(8);
  buffer.writeBigUInt64LE(BigInt(value));
  return buffer;
}

/// Tag 0. Data: [total_amount: 8][chunk_amount: 8][interval_ms: 8][fee_reserve_lamports: 8][bump: 1][escrow_bump: 1].
export function initializeStream(
  sender: PublicKey,
  recipient: PublicKey,
  mint: PublicKey,
  sourceToken: PublicKey,
  destinationToken: PublicKey,
  escrowToken: PublicKey,
  escrowAuthority: PublicKey,
  stream: PublicKey,
  validator: PublicKey,
  totalAmount: bigint,
  chunkAmount: bigint,
  intervalMs: bigint,
  feeReserveLamports: bigint,
  bump: number,
  escrowBump: number,
): TransactionInstruction {
  const data = Buffer.concat([
    discriminator(0),
    encodeU64(totalAmount),
    encodeU64(chunkAmount),
    encodeU64(intervalMs),
    encodeU64(feeReserveLamports),
    Buffer.from([bump, escrowBump]),
  ]);
  const accounts: AccountMeta[] = [
    { pubkey: sender, isSigner: true, isWritable: true },
    { pubkey: recipient, isSigner: false, isWritable: false },
    { pubkey: mint, isSigner: false, isWritable: false },
    { pubkey: sourceToken, isSigner: false, isWritable: true },
    { pubkey: destinationToken, isSigner: false, isWritable: false },
    { pubkey: escrowToken, isSigner: false, isWritable: true },
    { pubkey: escrowAuthority, isSigner: false, isWritable: false },
    { pubkey: stream, isSigner: false, isWritable: true },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    { pubkey: validator, isSigner: false, isWritable: false },
  ];
  return new TransactionInstruction({ programId: PROGRAM_ID, keys: accounts, data });
}

/// Tag 1. Data: (empty).
export function delegateStream(sender: PublicKey, stream: PublicKey, validator: PublicKey): TransactionInstruction {
  const accounts: AccountMeta[] = [
    { pubkey: sender, isSigner: true, isWritable: true },
    { pubkey: stream, isSigner: false, isWritable: true },
    { pubkey: PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: delegationBufferPda(stream), isSigner: false, isWritable: true },
    { pubkey: delegationRecordPda(stream), isSigner: false, isWritable: true },
    { pubkey: delegationMetadataPda(stream), isSigner: false, isWritable: true },
    { pubkey: DLP_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    { pubkey: validator, isSigner: false, isWritable: false },
  ];
  return new TransactionInstruction({ programId: PROGRAM_ID, keys: accounts, data: discriminator(1) });
}

/// Tag 2. Data: (empty).
export function scheduleStream(
  stream: PublicKey,
  crankSigner: PublicKey,
  escrowAuthority: PublicKey,
  validator: PublicKey,
): TransactionInstruction {
  const accounts: AccountMeta[] = [
    { pubkey: stream, isSigner: false, isWritable: true },
    { pubkey: crankSigner, isSigner: false, isWritable: false },
    { pubkey: delegationRecordPda(stream), isSigner: false, isWritable: false },
    { pubkey: escrowAuthority, isSigner: false, isWritable: false },
    { pubkey: MAGIC_CONTEXT_ID, isSigner: false, isWritable: true },
    { pubkey: MAGIC_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: magicFeeVaultPda(validator), isSigner: false, isWritable: true },
  ];
  return new TransactionInstruction({ programId: PROGRAM_ID, keys: accounts, data: discriminator(2) });
}

function encodeU64(value: bigint): Buffer {
  const buffer = Buffer.alloc(8);
  buffer.writeBigUInt64LE(value);
  return buffer;
}
