use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey, *};

use crate::states::{helper::AccountData, staking_pool_account::{StakingPool, PoolStatusEnum}};

pub fn process_resume_pool(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {

    let [authority, staking_pool_account] = accounts else {
        return Err(ProgramError::InvalidAccountData);
    };

    if !authority.is_signer() {
        return Err(ProgramError::InvalidAccountData);
    };

    let pool_id = u64::from_le_bytes(
        instruction_data[0..8].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );

    let (staking_pool_pda, _staking_pool_bump) = pubkey::find_program_address(
        &[
            b"staking_pool",
            authority.key().as_ref(),
            pool_id.to_le_bytes().as_ref(),
        ],
        &crate::ID, 
    );

    if *staking_pool_account.key() != staking_pool_pda {
        return Err(ProgramError::InvalidAccountData);
    };

    let mut staking_pool_account_info = StakingPool::from_account_info_mut(staking_pool_account)?;

    if staking_pool_account_info.authority != *authority.key() {
        return Err(ProgramError::InvalidAccountData);
    };

    let current_status = PoolStatusEnum::try_from(&staking_pool_account_info.pool_status)?;
    match current_status {
        PoolStatusEnum::Paused => {
            staking_pool_account_info.pool_status = PoolStatusEnum::Active as u8;
        },
        PoolStatusEnum::Active => {
            return Ok(());
        },
        _ => {
            return Err(ProgramError::InvalidAccountData);
        }
    }

    Ok(())
}