use pinocchio::{account_info::AccountInfo, program_error::ProgramError ,*};
use pinocchio_token::{instructions::TransferChecked, state::{Mint, TokenAccount}};
use crate::states::{global_config::GlobalConfig, helper::AccountData, staking_pool_account::StakingPool};

pub fn process_fund_reward_vault(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {

    let [
        authority,
        authority_token_account,
        reward_token_mint,
        reward_token_vault,
        staking_pool_account,
        global_config_account,
        token_program
    ] = accounts else {
        return Err(ProgramError::InvalidAccountData);
    };

    if !authority.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    };

    if instruction_data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    };

    let reward_amount = u64::from_le_bytes(
        instruction_data[0..8].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );

    if reward_amount == 0 {
        return Err(ProgramError::InvalidArgument);
    };

    let staking_pool_info = StakingPool::from_account_info(staking_pool_account)?;

    if staking_pool_info.reward_token_mint != *reward_token_mint.key() {
        return Err(ProgramError::InvalidAccountData);
    };

    if staking_pool_info.reward_token_vault != *reward_token_vault.key() {
        return Err(ProgramError::InvalidAccountData);
    };

    // if staking_pool_info.emergency_pause_flag {
    //     return Err(ProgramError::InvalidAccountData);
    // };

    let authority_token_info = TokenAccount::from_account_info(authority_token_account)?;
    let reward_vault_info = TokenAccount::from_account_info(reward_token_vault)?;

    if !authority_token_info.is_initialized() {
        return Err(ProgramError::InvalidAccountData);
    };

    if *authority_token_info.mint() != *reward_token_mint.key() {
        return Err(ProgramError::InvalidAccountData);
    };

    if *authority_token_info.owner() != *authority.key() {
        return Err(ProgramError::InvalidAccountData);
    };

    if *reward_vault_info.mint() != *reward_token_mint.key() {
        return Err(ProgramError::InvalidAccountData);
    };

    if *reward_vault_info.owner() != *global_config_account.key() {
        return Err(ProgramError::InvalidAccountData);
    };

    if authority_token_info.amount() < reward_amount {
        return Err(ProgramError::InsufficientFunds);
    };

    let reward_mint_account_info = Mint::from_account_info(reward_token_mint)?;

    TransferChecked {
        from: authority_token_account,
        to: reward_token_vault,
        mint: reward_token_mint,
        authority: authority,
        amount: reward_amount,
        decimals: reward_mint_account_info.decimals(),
    }.invoke()?;

    Ok(())
}
