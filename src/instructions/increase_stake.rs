use pinocchio::{account_info::AccountInfo, program_error::ProgramError, instruction::Signer, sysvars::{clock::Clock, Sysvar}, *};
use pinocchio_token::{state::{TokenAccount, Mint}, instructions::{TransferChecked, MintToChecked}};

use crate::states::{helper::AccountData, staking_pool_account::StakingPool, user_stake_account::UserStakeAccount, global_config::GlobalConfig};

pub fn process_increase_stake(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    let [
        user,  
        authority,              // Add this for global config authority
        stake_token_mint,
        stake_token_vault,
        liquid_stake_mint,
        global_config_account,  // Add this
        staking_pool_account,
        user_token_account,
        user_stake_account,
        user_lst_token_account,
        token_program
    ] = accounts else {
        return Err(ProgramError::InvalidAccountData);
    };

    if !user.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if instruction_data.len() < 16 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let pool_id = u64::from_le_bytes(
        instruction_data[0..8].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );

    let stake_amount = u64::from_le_bytes(
        instruction_data[8..16].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );

    if stake_amount == 0 {
        return Err(ProgramError::InvalidArgument);
    }

    // Get global config for mint authority
    let (global_config_pda, global_config_bump) = pubkey::find_program_address(
        &[b"global_config_account", authority.key().as_ref()],
        &crate::ID
    );

    if *global_config_account.key() != global_config_pda {
        return Err(ProgramError::InvalidAccountData);
    }

    let global_config = GlobalConfig::from_account_info(global_config_account)?;
    let mut staking_pool = StakingPool::from_account_info_mut(staking_pool_account)?;
    let mut user_stake = UserStakeAccount::from_account_info_mut(user_stake_account)?;

    if staking_pool.pool_status != 0 {
        return Err(ProgramError::InvalidAccountData);
    }

    if staking_pool.emergency_pause_flag {
        return Err(ProgramError::InvalidAccountData);
    }

    if global_config.emergency_pause {
        return Err(ProgramError::InvalidAccountData);
    }

    if stake_amount < staking_pool.minimum_stake_amount {
        return Err(ProgramError::InvalidArgument);
    }

    if staking_pool.maximum_stake_limit > 0 && 
       staking_pool.total_staked.saturating_add(stake_amount) > staking_pool.maximum_stake_limit {
        return Err(ProgramError::InvalidArgument);
    }

    let user_token_info = TokenAccount::from_account_info(user_token_account)?;
    let stake_vault_info = TokenAccount::from_account_info(stake_token_vault)?;
    let user_lst_token_info = TokenAccount::from_account_info(user_lst_token_account)?;

    // Validation checks
    if *user_token_info.owner() != *user.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    if *user_token_info.mint() != *stake_token_mint.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    if user_token_info.amount() < stake_amount {
        return Err(ProgramError::InsufficientFunds);
    }

    if *stake_vault_info.mint() != staking_pool.stake_token_mint {
        return Err(ProgramError::InvalidAccountData);
    }

    if *user_lst_token_info.owner() != *user.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    if *user_lst_token_info.mint() != *liquid_stake_mint.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    if user_stake.owner != *user.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Find the existing position for this pool
    let mut position_index = None;
    for (i, position) in user_stake.positions.iter().enumerate() {
        if position.is_active && position.pool_id == pool_id {
            position_index = Some(i);
            break;
        }
    }

    let position_idx = position_index.ok_or(ProgramError::Custom(1001))?; // Position not found

    let current_timestamp = Clock::get()?.unix_timestamp;
    
    // Update pool rewards before modifying stakes
    update_pool_rewards(&mut staking_pool, current_timestamp)?;

    // Calculate LST tokens to mint (1:1 ratio in this case)
    let lst_tokens = stake_amount;

    // Transfer stake tokens from user to vault
    let stake_mint_info = Mint::from_account_info(stake_token_mint)?;
    
    TransferChecked {
        from: user_token_account,
        to: stake_token_vault,
        mint: stake_token_mint,
        authority: user,
        amount: stake_amount,
        decimals: stake_mint_info.decimals(),
    }.invoke()?;

    // Mint LST tokens to user
    let global_config_bump_arr = &[global_config_bump];
    let seeds = seeds!(
        b"global_config_account", 
        authority.key().as_ref(),
        global_config_bump_arr
    );
    let signer_seeds = Signer::from(&seeds);

    let mint_account_info = Mint::from_account_info(liquid_stake_mint)?;

    MintToChecked {
        mint: liquid_stake_mint,
        account: user_lst_token_account,
        mint_authority: global_config_account,
        amount: lst_tokens,
        decimals: mint_account_info.decimals()
    }.invoke_signed(&[signer_seeds])?;

    // Update the existing position
    user_stake.positions[position_idx].staked_amount = 
        user_stake.positions[position_idx].staked_amount.saturating_add(stake_amount);
    user_stake.positions[position_idx].lst_tokens = 
        user_stake.positions[position_idx].lst_tokens.saturating_add(lst_tokens);
    user_stake.positions[position_idx].last_reward_update = current_timestamp;

    // Update user stake totals
    user_stake.total_staked_amount = user_stake.total_staked_amount.saturating_add(stake_amount);
    user_stake.total_lst_balance = user_stake.total_lst_balance.saturating_add(lst_tokens);
    user_stake.last_update_timestamp = current_timestamp;

    // Update pool totals
    staking_pool.total_staked = staking_pool.total_staked.saturating_add(stake_amount);
    staking_pool.liquid_stake_supply = staking_pool.liquid_stake_supply.saturating_add(lst_tokens);

    Ok(())
}

fn update_pool_rewards(pool: &mut StakingPool, current_timestamp: i64) -> ProgramResult {
    if pool.total_staked == 0 {
        return Ok(());
    }

    let time_elapsed = current_timestamp.saturating_sub(pool.creation_timestamp).max(0) as u64;
    if time_elapsed == 0 {
        return Ok(());
    }

    let rewards_earned = (pool.reward_rate_per_second as u128)
        .saturating_mul(time_elapsed as u128);
    
    let scaling_factor = 1_000_000_000_000u128; 
    let reward_per_share = rewards_earned
        .saturating_mul(scaling_factor)
        .saturating_div(pool.total_staked as u128);

    pool.accumulated_reward_per_share = pool.accumulated_reward_per_share
        .saturating_add(reward_per_share);

    Ok(())
}