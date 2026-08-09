//! Program-wide constants: PDA seeds, wire limits, and USDC unit conversion.

/// Stream state PDA seed prefix.
/// Full seeds: `[STREAM_SEED, sender, recipient, mint, stream_id (LE), bump]`.
pub const STREAM_SEED: &[u8] = b"stream";

/// Program address deployed by the local Surfpool Infrastructure-as-Code runbook.
pub const PROGRAM_ID_STR: &str = "2iGXTHjaBJW6auyKm7V3ZcbBcQMfxHJfVoucx8XhCH6V";

/// Classic USDC-compatible decimal count.
pub const USDC_DECIMALS: u8 = 6;
/// Integer base units in one whole USDC.
pub const USDC_SCALE: u64 = 1_000_000;

pub const DEMO_TOTAL_USDC: u64 = 1_000_000;
pub const DEMO_CHUNK_USDC: u64 = 100;
pub const DEMO_TOTAL_BASE_UNITS: u64 = 1_000_000_000_000;
pub const DEMO_CHUNK_BASE_UNITS: u64 = 100_000_000;
pub const DEMO_ITERATIONS: u64 = 10_000;

/// Converts whole USDC to base units without floating point.
pub const fn whole_usdc_to_base_units(whole: u64) -> Option<u64> {
    whole.checked_mul(USDC_SCALE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ceil_div;

    #[test]
    fn demo_constants_are_exact() {
        assert_eq!(
            whole_usdc_to_base_units(DEMO_TOTAL_USDC),
            Some(DEMO_TOTAL_BASE_UNITS)
        );
        assert_eq!(
            whole_usdc_to_base_units(DEMO_CHUNK_USDC),
            Some(DEMO_CHUNK_BASE_UNITS)
        );
        assert_eq!(
            ceil_div(DEMO_TOTAL_BASE_UNITS, DEMO_CHUNK_BASE_UNITS),
            Some(DEMO_ITERATIONS)
        );
    }

    #[test]
    fn whole_amount_overflow_is_rejected() {
        assert_eq!(whole_usdc_to_base_units(u64::MAX), None);
    }
}
