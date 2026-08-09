//! Magic Action provenance validation.
//!
//! Confirms a base-layer instruction was genuinely invoked by the DLP's secure
//! CallHandler-v2 suffix for *this* program's own scheduled action, not forged input.

use ephemeral_rollups_pinocchio::pda::ephemeral_balance_pda_from_payer;
use pinocchio::{error::ProgramError, AccountView, Address};

use crate::error::StreamError;

/// Validate the secure CallHandler-v2 suffix appended by the DLP on the base layer.
pub fn validate_action_suffix(
    program_id: &Address,
    expected_action_authority: &Address,
    source_program: &AccountView,
    action_escrow_authority: &AccountView,
    action_escrow_signer: &AccountView,
) -> Result<(), ProgramError> {
    let expected_action_escrow = ephemeral_balance_pda_from_payer(expected_action_authority, 0);
    if source_program.address() != program_id
        || !source_program.executable()
        || action_escrow_authority.address() != expected_action_authority
        || action_escrow_signer.address() != &expected_action_escrow
        || !action_escrow_signer.is_signer()
        || !action_escrow_signer.owned_by(&pinocchio_system::ID)
    {
        return Err(StreamError::InvalidActionSource.into());
    }
    Ok(())
}
