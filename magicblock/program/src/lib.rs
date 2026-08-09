//! Pinocchio implementation of a locally scheduled USDC payment stream.
//!
//! The stream state is delegated to a MagicBlock Ephemeral Rollup. Native Cranks
//! call `release_chunk` there, and each call schedules a Magic Action that settles
//! exactly one SPL-token transfer on the Surfpool base layer.

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
mod validation;

use crate::instructions::StreamInstruction;

/// Program id shared with the checked-in local deployment keypair.
pub const ID: Address =
    Address::new_from_array(pubkey!("J6JPeaFMpp9hoha6KGfG2tWTWhAqdtJtWJwrNYDW9SFx"));

no_allocator!();
nostd_panic_handler!();

pinocchio::program_entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Address,
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let (instruction, payload) = instructions::decode(data)?;

    match instruction {
        StreamInstruction::Initialize => instructions::initialize::process(program_id, accounts, payload),
        StreamInstruction::Delegate => instructions::delegate::process(program_id, accounts, payload),
        StreamInstruction::Schedule => instructions::schedule::process(program_id, accounts, payload),
        StreamInstruction::Release => instructions::release::process(program_id, accounts, payload),
        StreamInstruction::SettleChunk => {
            instructions::settle_chunk::process(program_id, accounts, payload)
        }
        StreamInstruction::Finalize => instructions::finalize::process(program_id, accounts, payload),
        StreamInstruction::UndelegateCallback => {
            instructions::undelegate::process(program_id, accounts, payload)
        }
    }
}
