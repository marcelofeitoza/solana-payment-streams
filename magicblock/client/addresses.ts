//! Canonical program ids and PDA derivations shared by every client command.
//! Must stay byte-for-byte in sync with magicblock/program/src/{constants,pda}.rs.

import { PublicKey } from "@solana/web3.js";

export const PROGRAM_ID = new PublicKey("J6JPeaFMpp9hoha6KGfG2tWTWhAqdtJtWJwrNYDW9SFx");
export const CRANK_PROGRAM_ID = new PublicKey("Crank11111111111111111111111111111111111111");
export const DLP_PROGRAM_ID = new PublicKey("DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh");
export const MAGIC_PROGRAM_ID = new PublicKey("Magic11111111111111111111111111111111111111");
export const MAGIC_CONTEXT_ID = new PublicKey("MagicContext1111111111111111111111111111111");
export const BPF_LOADER_UPGRADEABLE_ID = new PublicKey("BPFLoaderUpgradeab1e11111111111111111111111");
export const TOKEN_PROGRAM_ID = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
export const LOCAL_VALIDATOR_STR = "mAGicPQYBMvcYveUZA5F5UNNwyHvfYh5xkLS2Fr1mev";

/// Seeds: ["stream", sender, recipient, mint].
export function streamPda(sender: PublicKey, recipient: PublicKey, mint: PublicKey) {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("stream"), sender.toBytes(), recipient.toBytes(), mint.toBytes()],
    PROGRAM_ID,
  );
}

/// Seeds: ["escrow", stream].
export function escrowAuthorityPda(stream: PublicKey) {
  return PublicKey.findProgramAddressSync([Buffer.from("escrow"), stream.toBytes()], PROGRAM_ID);
}

/// Seeds: ["crank-executor", stream], under the Crank program.
export function crankSignerPda(stream: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("crank-executor"), stream.toBytes()],
    CRANK_PROGRAM_ID,
  )[0];
}

/// Seeds: ["buffer", stream], under this program (the delegation CPI's temporary buffer).
export function delegationBufferPda(stream: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync([Buffer.from("buffer"), stream.toBytes()], PROGRAM_ID)[0];
}

/// Seeds: ["delegation", stream], under the DLP.
export function delegationRecordPda(stream: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync([Buffer.from("delegation"), stream.toBytes()], DLP_PROGRAM_ID)[0];
}

/// Seeds: ["delegation-metadata", stream], under the DLP.
export function delegationMetadataPda(stream: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("delegation-metadata"), stream.toBytes()],
    DLP_PROGRAM_ID,
  )[0];
}

/// Seeds: ["v-fees-vault", validator], under the DLP.
export function validatorFeeVaultPda(validator: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("v-fees-vault"), validator.toBytes()],
    DLP_PROGRAM_ID,
  )[0];
}

/// Seeds: ["fees-vault"], under the DLP.
export function protocolFeeVaultPda(): PublicKey {
  return PublicKey.findProgramAddressSync([Buffer.from("fees-vault")], DLP_PROGRAM_ID)[0];
}

/// Seeds: ["magic-fee-vault", validator], under the DLP.
export function magicFeeVaultPda(validator: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("magic-fee-vault"), validator.toBytes()],
    DLP_PROGRAM_ID,
  )[0];
}

/// Seeds: ["balance", payer, [index]], under the DLP. The base-layer escrow that funds one
/// stream's scheduled Magic Actions.
export function ephemeralBalancePda(payer: PublicKey, index: number): PublicKey {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("balance"), payer.toBytes(), Buffer.from([index])],
    DLP_PROGRAM_ID,
  )[0];
}

/// The DLP's own ProgramData account (standard BPFLoaderUpgradeable derivation).
export function delegationProgramDataPda(): PublicKey {
  return PublicKey.findProgramAddressSync([DLP_PROGRAM_ID.toBytes()], BPF_LOADER_UPGRADEABLE_ID)[0];
}
