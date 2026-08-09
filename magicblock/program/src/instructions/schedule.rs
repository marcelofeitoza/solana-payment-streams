//! Schedule instruction (Ephemeral Rollup, permissionless)
//!
//! Registers a repeating Native Crank job that calls `release_chunk` on this stream for
//! every remaining iteration, at the stream's configured interval. Relayer-paid; anyone
//! may submit it once, but never twice for the same stream.

use ephemeral_rollups_pinocchio::{
    consts::{MAGIC_CONTEXT_ID, MAGIC_PROGRAM_ID},
    crank::{CrankInstruction, ScheduleCrankArgs, ScheduleCrankCpi},
    pda::magic_fee_vault_pda_from_validator,
};
use pinocchio::{
    cpi::Signer, error::ProgramError, instruction::InstructionAccount, AccountView, Address,
    ProgramResult,
};

use crate::{
    constants::{RELEASE_CHUNK, SCHEDULE_DATA_BUFFER_LEN},
    error::StreamError,
    pda,
    state::StreamState,
};

/// Accounts:
/// 0. `[writable]` stream
/// 1. `[]` crank_signer - Native Crank executor PDA for this stream
/// 2. `[]` delegation_record
/// 3. `[]` escrow_authority
/// 4. `[writable]` magic_context
/// 5. `[]` magic_program
/// 6. `[writable]` magic_fee_vault
///
/// Data: (empty)
pub fn process(program_id: &Address, accounts: &[AccountView], data: &[u8]) -> ProgramResult {
    if !data.is_empty() {
        return Err(StreamError::InvalidInstruction.into());
    }
    let [stream, crank_signer, delegation_record, escrow_authority, magic_context, magic_program, magic_fee_vault] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let mut state = StreamState::load(stream)?;
    state.validate_pdas(program_id, stream, escrow_authority)?;
    state.validate_delegation(stream, delegation_record)?;
    if !state.active {
        return Err(StreamError::StreamInactive.into());
    }
    if state.scheduled {
        return Err(StreamError::StreamAlreadyScheduled.into());
    }
    let validator = Address::new_from_array(state.validator);
    if magic_program.address() != &MAGIC_PROGRAM_ID
        || magic_context.address() != &MAGIC_CONTEXT_ID
        || magic_fee_vault.address() != &magic_fee_vault_pda_from_validator(&validator)
        || crank_signer.address() != &pda::crank_signer_address(stream.address())
    {
        return Err(StreamError::InvalidPda.into());
    }

    let iterations = state.remaining_iterations().ok_or(StreamError::InvalidAmount)?;
    let interval = i64::try_from(state.interval_ms).map_err(|_| StreamError::InvalidAmount)?;
    let iterations = i64::try_from(iterations).map_err(|_| StreamError::InvalidAmount)?;

    let release_accounts = [
        InstructionAccount::writable(stream.address()),
        InstructionAccount::readonly_signer(crank_signer.address()),
        InstructionAccount::readonly(delegation_record.address()),
        InstructionAccount::readonly(escrow_authority.address()),
        InstructionAccount::writable(magic_context.address()),
        InstructionAccount::readonly(magic_program.address()),
        InstructionAccount::writable(magic_fee_vault.address()),
    ];
    let release_instruction = [CrankInstruction::new(
        *program_id,
        &release_accounts,
        &RELEASE_CHUNK,
    )];
    let schedule_accounts = [
        crank_signer,
        delegation_record,
        escrow_authority,
        magic_context,
        magic_program,
        magic_fee_vault,
    ];
    let args = ScheduleCrankArgs::new(state.task_id, &release_instruction)
        .execution_interval_millis(interval)
        .iterations(iterations);

    state.scheduled = true;
    state.store(stream)?;

    let signer_seeds = pda::stream_signer_seeds(&state);
    let mut schedule_data = [0_u8; SCHEDULE_DATA_BUFFER_LEN];
    ScheduleCrankCpi::new(*stream, *magic_program, &schedule_accounts, args)
        .invoke_signed::<7>(&mut schedule_data, &[Signer::from(&signer_seeds)])
}
