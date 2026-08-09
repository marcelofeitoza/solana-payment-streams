//! Compact custom errors returned by program validation and arithmetic checks.

use pinocchio::error::ProgramError;

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamError {
    InvalidInstruction = 0x5354_0001,
    InvalidState = 0x5354_0002,
    InvalidPda = 0x5354_0003,
    InvalidAuthority = 0x5354_0004,
    InvalidValidator = 0x5354_0005,
    InvalidTokenAccount = 0x5354_0006,
    InvalidAmount = 0x5354_0007,
    StreamInactive = 0x5354_0008,
    StreamAlreadyScheduled = 0x5354_0009,
    StreamNotScheduled = 0x5354_000a,
    InvalidActionSource = 0x5354_000b,
    ArithmeticOverflow = 0x5354_000c,
}

impl From<StreamError> for ProgramError {
    fn from(value: StreamError) -> Self {
        ProgramError::Custom(value as u32)
    }
}
