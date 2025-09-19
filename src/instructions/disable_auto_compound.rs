use pinocchio::{account_info::AccountInfo, program_error::ProgramError, sysvars::{clock::Clock, Sysvar}, *};
use crate::states::{helper::AccountData, user_stake_account::UserStakeAccount};

pub fn process_disable_auto_compound(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    let [
        user,
        user_stake_account,
    ] = accounts else {
        return Err(ProgramError::InvalidAccountData);
    };

    if !user.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if instruction_data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let pool_id = u64::from_le_bytes(
        instruction_data[0..8].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );

    let mut user_stake = UserStakeAccount::from_account_info_mut(user_stake_account)?;

    if user_stake.owner != *user.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Find the position and disable auto-compound
    let mut position_found = false;
    for position in user_stake.positions.iter_mut() {
        if position.is_active && position.pool_id == pool_id {
            position.auto_compound_enabled = false;
            position_found = true;
            break;
        }
    }

    if !position_found {
        return Err(ProgramError::Custom(1002));
    }

    let current_timestamp = Clock::get()?.unix_timestamp;
    user_stake.last_update_timestamp = current_timestamp;

    Ok(())
}