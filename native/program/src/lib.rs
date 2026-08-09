//! Pinocchio implementation of a base-layer USDC payment stream.
//!
//! The program never schedules work. A permissionless off-chain keeper submits ordinary
//! transactions, while this program exclusively controls custody and validates every release.

#![no_std]
#![allow(unexpected_cfgs)]

use pinocchio::{no_allocator, nostd_panic_handler, AccountView, Address, ProgramResult};
use pinocchio_pubkey::pubkey;

pub mod constants;
mod error;
pub mod instructions;
pub mod pda;
pub mod state;
pub mod token;

use crate::instructions::StreamInstruction;

/// Address installed by `surfpool/deployment/main.tx`.
pub const ID: Address =
    Address::new_from_array(pubkey!("2iGXTHjaBJW6auyKm7V3ZcbBcQMfxHJfVoucx8XhCH6V"));

no_allocator!();
nostd_panic_handler!();

pinocchio::program_entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Address,
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let (tag, payload) = data
        .split_first()
        .ok_or(error::StreamError::InvalidInstruction)?;

    match StreamInstruction::try_from(tag)? {
        StreamInstruction::Initialize => {
            instructions::initialize::process(program_id, accounts, payload)
        }
        StreamInstruction::Release => {
            instructions::release::process(program_id, accounts, payload)
        }
    }
}
