//! Stable custom program errors returned for protocol-level validation failures.

use pinocchio::error::ProgramError;

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamError {
    InvalidInstruction = 0x4e55_0001,
    InvalidState = 0x4e55_0002,
    InvalidPda = 0x4e55_0003,
    InvalidTokenAccount = 0x4e55_0005,
    InvalidMint = 0x4e55_0006,
    InvalidAmount = 0x4e55_0007,
    InvalidStatus = 0x4e55_0008,
    ReleaseNotDue = 0x4e55_0009,
    FutureChunkIndex = 0x4e55_000a,
    ArithmeticOverflow = 0x4e55_000b,
    InvalidTimestamp = 0x4e55_000d,
}

impl From<StreamError> for ProgramError {
    fn from(value: StreamError) -> Self {
        ProgramError::Custom(value as u32)
    }
}
