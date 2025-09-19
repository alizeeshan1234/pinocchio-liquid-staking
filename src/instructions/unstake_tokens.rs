use pinocchio::{account_info::AccountInfo, program_error::ProgramError, instruction::Signer, sysvars::{clock::Clock, Sysvar}, *};
use pinocchio_token::{state::{TokenAccount, Mint}, instructions::{TransferChecked, BurnChecked}};

use crate::states::{helper::AccountData, staking_pool_account::StakingPool, user_stake_account::UserStakeAccount, global_config::GlobalConfig};

pub fn process_unstake(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    let [
        user,                    
        authority,              
        stake_token_mint,       
        stake_token_vault,      
        liquid_stake_mint,      
        global_config_account,  
        staking_pool_account,   
        user_token_account,     
        user_stake_account,     
        user_lst_token_account, 
        token_program,          
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

    let lst_amount = u64::from_le_bytes(
        instruction_data[8..16].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );

    if lst_amount == 0 {
        return Err(ProgramError::InvalidArgument);
    }

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

    let mut position_index = None;
    for (i, position) in user_stake.positions.iter().enumerate() {
        if position.is_active && position.pool_id == pool_id {
            position_index = Some(i);
            break;
        }
    }

    let position_idx = position_index.ok_or(ProgramError::Custom(1002))?; 
    let position = &user_stake.positions[position_idx];

    if position.lst_tokens < lst_amount {
        return Err(ProgramError::InsufficientFunds);
    }

    let current_timestamp = Clock::get()?.unix_timestamp;

    if position.lock_exipry_enable && current_timestamp < position.lock_expiry {
        let penalty_amount = calculate_early_withdrawal_penalty(
            lst_amount, 
            staking_pool.early_withdraw_penalty
        )?;
    }

    // Calculate how many underlying tokens to return (1:1 ratio in basic case)
    let underlying_tokens = lst_amount;

    // Validate token accounts
    let user_token_info = TokenAccount::from_account_info(user_token_account)?;
    let stake_vault_info = TokenAccount::from_account_info(stake_token_vault)?;
    let user_lst_token_info = TokenAccount::from_account_info(user_lst_token_account)?;

    if *user_token_info.owner() != *user.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    if *user_lst_token_info.owner() != *user.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    if user_lst_token_info.amount() < lst_amount {
        return Err(ProgramError::InsufficientFunds);
    }

    // Update pool rewards before processing unstake
    update_pool_rewards(&mut staking_pool, current_timestamp)?;

    // Calculate and distribute any pending rewards
    calculate_and_distribute_rewards(
        &mut user_stake.positions[position_idx],
        &staking_pool,
        current_timestamp
    )?;

    // Burn LST tokens from user
    let lst_mint_info = Mint::from_account_info(liquid_stake_mint)?;
    
    BurnChecked {
        mint: liquid_stake_mint,
        account: user_lst_token_account,
        authority: user,
        amount: lst_amount,
        decimals: lst_mint_info.decimals(),
    }.invoke()?;

    // Transfer underlying tokens back to user from vault
    let global_config_bump_arr = &[global_config_bump];
    let seeds = seeds!(
        b"global_config_account", 
        authority.key().as_ref(),
        global_config_bump_arr
    );
    let signer_seeds = Signer::from(&seeds);

    let stake_mint_info = Mint::from_account_info(stake_token_mint)?;

    TransferChecked {
        from: stake_token_vault,
        to: user_token_account,
        mint: stake_token_mint,
        authority: global_config_account, // Vault is owned by global config
        amount: underlying_tokens,
        decimals: stake_mint_info.decimals(),
    }.invoke_signed(&[signer_seeds])?;

    // Update user's position
    user_stake.positions[position_idx].staked_amount = 
        user_stake.positions[position_idx].staked_amount.saturating_sub(underlying_tokens);
    user_stake.positions[position_idx].lst_tokens = 
        user_stake.positions[position_idx].lst_tokens.saturating_sub(lst_amount);
    user_stake.positions[position_idx].last_reward_update = current_timestamp;

    if user_stake.positions[position_idx].lst_tokens == 0 {
        user_stake.positions[position_idx].is_active = false;
        user_stake.active_positions = user_stake.active_positions.saturating_sub(1);
    }

    user_stake.total_staked_amount = user_stake.total_staked_amount.saturating_sub(underlying_tokens);
    user_stake.total_lst_balance = user_stake.total_lst_balance.saturating_sub(lst_amount);
    user_stake.last_update_timestamp = current_timestamp;

    staking_pool.total_staked = staking_pool.total_staked.saturating_sub(underlying_tokens);
    staking_pool.liquid_stake_supply = staking_pool.liquid_stake_supply.saturating_sub(lst_amount);

    Ok(())
}

fn calculate_early_withdrawal_penalty(amount: u64, penalty_rate: u64) -> Result<u64, ProgramError> {
    let penalty = (amount as u128)
        .saturating_mul(penalty_rate as u128)
        .saturating_div(10000u128) as u64; 
    
    Ok(penalty)
}

fn calculate_and_distribute_rewards(
    position: &mut crate::states::user_stake_account::StakePosition,
    pool: &StakingPool,
    current_timestamp: i64
) -> ProgramResult {
    let time_staked = current_timestamp.saturating_sub(position.last_reward_update);
    if time_staked > 0 {
        let rewards = calculate_position_rewards(position, pool, time_staked)?;
        position.pending_rewards = position.pending_rewards.saturating_add(rewards);
    }
    
    Ok(())
}

fn calculate_position_rewards(
    position: &crate::states::user_stake_account::StakePosition,
    pool: &StakingPool,
    time_elapsed: i64
) -> Result<u64, ProgramError> {
    if position.staked_amount == 0 || time_elapsed <= 0 {
        return Ok(0);
    }

    let rewards = (position.staked_amount as u128)
        .saturating_mul(pool.reward_rate_per_second as u128)
        .saturating_mul(time_elapsed as u128)
        .saturating_div(1_000_000_000_000u128) as u64; 

    Ok(rewards)
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