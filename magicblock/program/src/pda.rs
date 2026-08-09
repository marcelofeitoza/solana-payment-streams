//! Canonical PDA derivation shared by on-chain validation and off-chain clients.

use pinocchio::{cpi::Seed, error::ProgramError, Address};
use pinocchio_pubkey::pubkey;

use crate::{
    constants::{CRANK_EXECUTOR_SEED, ESCROW_AUTHORITY_SEED, STREAM_SEED},
    error::StreamError,
    state::StreamState,
};

pub const CRANK_PROGRAM_ID: Address =
    Address::new_from_array(pubkey!("Crank11111111111111111111111111111111111111"));

/// Stream state PDA. Seeds: `["stream", sender, recipient, mint, bump]`.
pub fn stream_address(program_id: &Address, state: &StreamState) -> Result<Address, ProgramError> {
    create_program_address(
        &[
            STREAM_SEED,
            &state.sender,
            &state.recipient,
            &state.mint,
            &[state.bump],
        ],
        program_id,
    )
}

pub fn stream_address_from_parts(
    program_id: &Address,
    sender: &Address,
    recipient: &Address,
    mint: &Address,
    bump: u8,
) -> Result<Address, ProgramError> {
    create_program_address(
        &[
            STREAM_SEED,
            sender.as_ref(),
            recipient.as_ref(),
            mint.as_ref(),
            &[bump],
        ],
        program_id,
    )
}

/// Non-delegated SPL escrow authority PDA. Seeds: `["escrow", stream, bump]`.
pub fn escrow_authority_address(
    program_id: &Address,
    stream: &Address,
    bump: u8,
) -> Result<Address, ProgramError> {
    create_program_address(&[ESCROW_AUTHORITY_SEED, stream.as_ref(), &[bump]], program_id)
}

/// Native Crank signer PDA, owned by the Crank program.
pub fn crank_signer_address(schedule_authority: &Address) -> Address {
    Address::find_program_address(
        &[CRANK_EXECUTOR_SEED, schedule_authority.as_ref()],
        &CRANK_PROGRAM_ID,
    )
    .0
}

/// Signer seeds for CPIs authorized by the stream PDA (`create_account`, crank scheduling,
/// Magic Action intents). Borrows directly from `state`, so no local seed buffers are needed.
pub fn stream_signer_seeds(state: &StreamState) -> [Seed<'_>; 5] {
    [
        Seed::from(STREAM_SEED),
        Seed::from(state.sender.as_ref()),
        Seed::from(state.recipient.as_ref()),
        Seed::from(state.mint.as_ref()),
        Seed::from(core::slice::from_ref(&state.bump)),
    ]
}

fn create_program_address(
    seeds: &[&[u8]],
    program_id: &Address,
) -> Result<Address, ProgramError> {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    {
        Address::create_program_address(seeds, program_id)
            .map_err(|_| StreamError::InvalidPda.into())
    }
    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    {
        use solana_pubkey::Pubkey;
        let program = Pubkey::new_from_array(*program_id.as_array());
        let derived = Pubkey::create_program_address(seeds, &program)
            .map_err(|_| ProgramError::from(StreamError::InvalidPda))?;
        Ok(Address::new_from_array(derived.to_bytes()))
    }
}
