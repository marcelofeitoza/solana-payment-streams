//! Program-wide constants: PDA seeds, wire ABI tags, and USDC unit conversion.
//!
//! Also consumed directly by the `magicblock-runner` host binary (path dependency in the
//! same workspace), so these names and values are part of the public wire contract.

/// Canonical local PoC program id.
pub const PROGRAM_ID_STR: &str = "J6JPeaFMpp9hoha6KGfG2tWTWhAqdtJtWJwrNYDW9SFx";
/// Current MagicBlock local validator identity.
pub const LOCAL_VALIDATOR_STR: &str = "mAGicPQYBMvcYveUZA5F5UNNwyHvfYh5xkLS2Fr1mev";
/// MagicBlock Native Crank program id.
pub const CRANK_PROGRAM_ID_STR: &str = "Crank11111111111111111111111111111111111111";

/// A local USDC mint uses the same six-decimal integer convention as production USDC.
pub const USDC_DECIMALS: u8 = 6;
/// Smallest integer units in one USDC. No floating-point conversion is used.
pub const USDC_SCALE: u64 = 1_000_000;
/// Demonstration total in whole USDC.
pub const DEFAULT_TOTAL_USDC: u64 = 1_000_000;
/// Demonstration chunk in whole USDC.
pub const DEFAULT_CHUNK_USDC: u64 = 100;
/// Demonstration total in base token units.
pub const DEFAULT_TOTAL_AMOUNT: u64 = DEFAULT_TOTAL_USDC * USDC_SCALE;
/// Demonstration chunk in base token units.
pub const DEFAULT_CHUNK_AMOUNT: u64 = DEFAULT_CHUNK_USDC * USDC_SCALE;
/// Number of Native Crank executions in the requested demonstration.
pub const DEFAULT_ITERATIONS: u64 = 10_000;
/// Lamports placed in each local fee reserve: the delegated stream fee balance
/// and the DLP base-action escrow derived from the stream PDA.
pub const DEFAULT_FEE_RESERVE_LAMPORTS: u64 = 5_000_000_000;
/// Stable task id because one stream PDA owns at most one scheduled crank.
pub const CRANK_TASK_ID: i64 = 0;

/// PDA seed for stream state.
pub const STREAM_SEED: &[u8] = b"stream";
/// PDA seed for the non-delegated SPL escrow authority.
pub const ESCROW_AUTHORITY_SEED: &[u8] = b"escrow";
/// Native Crank signer PDA seed, confirmed against the MagicBlock program API.
pub const CRANK_EXECUTOR_SEED: &[u8] = b"crank-executor";

/// Program instruction tags. Plain native-Rust 8-byte tags, not Anchor discriminators.
pub const INITIALIZE_STREAM: [u8; 8] = 0_u64.to_le_bytes();
pub const DELEGATE_STREAM: [u8; 8] = 1_u64.to_le_bytes();
pub const SCHEDULE_STREAM: [u8; 8] = 2_u64.to_le_bytes();
pub const RELEASE_CHUNK: [u8; 8] = 3_u64.to_le_bytes();
pub const SETTLE_CHUNK: [u8; 8] = 4_u64.to_le_bytes();
pub const FINALIZE_STREAM: [u8; 8] = 5_u64.to_le_bytes();
/// The DLP-defined callback tag used when restoring an undelegated account.
pub const UNDELEGATION_CALLBACK: [u8; 8] = [196, 28, 41, 206, 48, 37, 51, 167];

/// Compute budget attached to every scheduled Magic Action.
pub const ACTION_COMPUTE_UNITS: u32 = 150_000;
/// Scratch buffer size for building a `MagicIntentBundleBuilder` payload.
pub const INTENT_DATA_BUFFER_LEN: usize = 1_024;
/// Scratch buffer size for building a `ScheduleCrankCpi` payload.
pub const SCHEDULE_DATA_BUFFER_LEN: usize = 512;

/// Convert whole USDC to base units with checked integer arithmetic.
pub const fn whole_usdc_to_base_units(whole_usdc: u64) -> Option<u64> {
    whole_usdc.checked_mul(USDC_SCALE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_constants_are_exact() {
        assert_eq!(
            whole_usdc_to_base_units(DEFAULT_TOTAL_USDC),
            Some(DEFAULT_TOTAL_AMOUNT)
        );
        assert_eq!(
            whole_usdc_to_base_units(DEFAULT_CHUNK_USDC),
            Some(DEFAULT_CHUNK_AMOUNT)
        );
    }

    #[test]
    fn whole_amount_overflow_is_rejected() {
        assert_eq!(whole_usdc_to_base_units(u64::MAX), None);
    }
}
