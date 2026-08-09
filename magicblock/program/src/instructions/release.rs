//! Release instruction (Ephemeral Rollup, Native Crank signer)
//!
//! Authorizes exactly one chunk and schedules its base-layer settlement as a Magic Action:
//! `SETTLE_CHUNK` for an ordinary chunk, or `FINALIZE_STREAM` (with commit-and-undelegate)
//! once the stream is fully sent. A replay after the final call is a successful no-op.

use core::cmp::min;

use ephemeral_rollups_pinocchio::{
    consts::{MAGIC_CONTEXT_ID, MAGIC_PROGRAM_ID},
    intent_bundle::{ActionArgs, CallHandler, MagicIntentBundleBuilder},
    pda::magic_fee_vault_pda_from_validator,
};
use pinocchio::{cpi::Signer, error::ProgramError, AccountView, Address, ProgramResult};

use crate::{
    constants::{ACTION_COMPUTE_UNITS, FINALIZE_STREAM, INTENT_DATA_BUFFER_LEN, SETTLE_CHUNK},
    error::StreamError,
    instructions::{finalize_accounts, settle_chunk_accounts},
    pda,
    state::StreamState,
};

/// Accounts:
/// 0. `[writable]` stream
/// 1. `[signer]` crank_signer - Native Crank executor PDA for this stream
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
    if crank_signer.address() != &pda::crank_signer_address(stream.address())
        || !crank_signer.is_signer()
    {
        return Err(StreamError::InvalidAuthority.into());
    }
    let validator = Address::new_from_array(state.validator);
    if magic_program.address() != &MAGIC_PROGRAM_ID
        || magic_context.address() != &MAGIC_CONTEXT_ID
        || magic_fee_vault.address() != &magic_fee_vault_pda_from_validator(&validator)
    {
        return Err(StreamError::InvalidPda.into());
    }
    if !state.scheduled {
        return Err(StreamError::StreamNotScheduled.into());
    }

    // A replay after the final successful call is deliberately a successful no-op.
    if !state.active || state.sent_amount >= state.total_amount {
        return Ok(());
    }

    let remaining = state
        .total_amount
        .checked_sub(state.sent_amount)
        .ok_or(StreamError::ArithmeticOverflow)?;
    let amount = min(remaining, state.chunk_amount);
    let new_sent = state
        .sent_amount
        .checked_add(amount)
        .ok_or(StreamError::ArithmeticOverflow)?;
    let is_final = new_sent == state.total_amount;
    state.sent_amount = new_sent;
    if is_final {
        state.active = false;
    }
    state.store(stream)?;

    let mut action_data = [0_u8; 24];
    action_data[..8].copy_from_slice(if is_final { &FINALIZE_STREAM } else { &SETTLE_CHUNK });
    action_data[8..16].copy_from_slice(&amount.to_le_bytes());
    action_data[16..24].copy_from_slice(&new_sent.to_le_bytes());

    let settle_accounts;
    let finalize_accounts_arr;
    let action_accounts: &[_] = if is_final {
        finalize_accounts_arr = finalize_accounts(&state, stream.address(), escrow_authority.address());
        &finalize_accounts_arr
    } else {
        settle_accounts = settle_chunk_accounts(&state, stream.address(), escrow_authority.address());
        &settle_accounts
    };
    let action = CallHandler {
        destination_program: *program_id,
        // The CallHandler authority must sign the Magic Program CPI. The stream PDA is also
        // the intent payer, so it is the appropriate action authority. The separate
        // escrow-authority PDA remains the SPL token authority and signs only inside the
        // base-layer action handler.
        escrow_authority: *stream,
        args: ActionArgs::new(&action_data).with_escrow_index(0),
        compute_units: ACTION_COMPUTE_UNITS,
        accounts: action_accounts,
        callback: None,
    };
    let actions = [action];
    let commit_accounts = [*stream];

    let signer_seeds = pda::stream_signer_seeds(&state);
    let signers = [Signer::from(&signer_seeds)];
    let mut intent_data = [0_u8; INTENT_DATA_BUFFER_LEN];
    let builder = MagicIntentBundleBuilder::new(*stream, *magic_context, *magic_program)
        .magic_fee_vault(*magic_fee_vault);

    if is_final {
        builder
            .commit_and_undelegate(&commit_accounts)
            .add_post_undelegate_actions(&actions)
            .fold()
            .build_and_invoke_signed(&mut intent_data, &signers)
    } else {
        builder
            .commit(&commit_accounts)
            .add_post_commit_actions(&actions)
            .build_and_invoke_signed(&mut intent_data, &signers)
    }
}
