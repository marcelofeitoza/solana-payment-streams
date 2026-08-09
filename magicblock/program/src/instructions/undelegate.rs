//! UndelegateCallback instruction (base layer, DLP callback)
//!
//! Restores a committed stream account when the DLP invokes its external callback. The
//! callback payload is DLP-defined and forwarded verbatim, with no length constraint here.

use ephemeral_rollups_pinocchio::instruction::undelegate;
use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};

/// Accounts:
/// 0. `[writable]` delegated_account
/// 1. `[writable]` buffer_account
/// 2. `[signer, writable]` payer
/// 3. `[]` system_program
///
/// Any further accounts the DLP appends are ignored.
///
/// Data: DLP-defined callback payload, forwarded verbatim.
pub fn process(program_id: &Address, accounts: &[AccountView], data: &[u8]) -> ProgramResult {
    let [delegated_account, buffer_account, payer, _system_program, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    undelegate(delegated_account, program_id, buffer_account, payer, data)
}
