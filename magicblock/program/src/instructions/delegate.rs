//! Delegate instruction (base layer, sender-invoked)
//!
//! Delegates the newly initialized stream state to the selected local Ephemeral Rollup
//! validator so its subsequent lifecycle can run through Native Crank on the ER.

use ephemeral_rollups_pinocchio::{consts::DELEGATION_PROGRAM_ID, instruction::delegate_account, types::DelegateConfig};
use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};

use crate::{constants::STREAM_SEED, error::StreamError, pda, state::StreamState};

/// Accounts:
/// 0. `[signer]` sender - must equal the stream's recorded sender
/// 1. `[writable]` stream - delegated in place
/// 2. `[]` owner_program - this program (required by the delegation CPI)
/// 3. `[writable]` delegation_buffer
/// 4. `[writable]` delegation_record
/// 5. `[writable]` delegation_metadata
/// 6. `[]` delegation_program
/// 7. `[]` system_program
/// 8. `[]` validator - must equal the stream's recorded validator
///
/// Data: (empty)
pub fn process(program_id: &Address, accounts: &[AccountView], data: &[u8]) -> ProgramResult {
    if !data.is_empty() {
        return Err(StreamError::InvalidInstruction.into());
    }
    let [sender, stream, owner_program, delegation_buffer, delegation_record, delegation_metadata, delegation_program, system_program, validator] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !sender.is_signer() || sender.address() == stream.address() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if owner_program.address() != program_id
        || !owner_program.executable()
        || delegation_program.address() != &DELEGATION_PROGRAM_ID
        || !delegation_program.executable()
        || system_program.address() != &pinocchio_system::ID
    {
        return Err(ProgramError::IncorrectProgramId);
    }

    let state = StreamState::load(stream)?;
    if sender.address() != &Address::new_from_array(state.sender)
        || validator.address() != &Address::new_from_array(state.validator)
        || stream.address() != &pda::stream_address(program_id, &state)?
        || !state.active
    {
        return Err(StreamError::InvalidAuthority.into());
    }

    let seeds: [&[u8]; 4] = [STREAM_SEED, &state.sender, &state.recipient, &state.mint];
    delegate_account(
        &[
            sender,
            stream,
            owner_program,
            delegation_buffer,
            delegation_record,
            delegation_metadata,
            system_program,
        ],
        &seeds,
        state.bump,
        DelegateConfig {
            commit_frequency_ms: u32::MAX,
            validator: Some(Address::new_from_array(state.validator)),
        },
    )
}
