use pinocchio::{
    account_info::AccountInfo, 
    program_error::ProgramError, 
    instruction::Signer, 
    sysvars::{clock::Clock, Sysvar}, 
    *
};
use pinocchio_token::{
    state::{TokenAccount, Mint}, 
    instructions::{TransferChecked, MintToChecked}
};

use crate::states::{
    helper::AccountData, 
    staking_pool_account::StakingPool, 
    user_stake_account::{UserStakeAccount, StakePosition},
    global_config::GlobalConfig
};

pub fn process_execute_auto_compound(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    let [
        executor,                  
        position_owner,            
        authority,                 
        reward_token_mint,         
        stake_token_mint,          
        reward_token_vault,        
        stake_token_vault,          
        liquid_stake_mint,          
        global_config_account,      
        staking_pool_account,       
        user_stake_account,         
        user_lst_token_account,     
        treasury_account,           
        token_program,              
    ] = accounts else {
        return Err(ProgramError::InvalidAccountData);
    };

    if instruction_data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let pool_id = u64::from_le_bytes(
        instruction_data[0..8].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );

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

    if user_stake.owner != *position_owner.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    if user_stake.is_paused {
        return Err(ProgramError::Custom(4001)); 
    }

    if global_config.emergency_pause {
        return Err(ProgramError::Custom(4002)); 
    }

    if staking_pool.emergency_pause_flag {
        return Err(ProgramError::Custom(4003)); 
    }

    let mut position_index = None;
    for (i, position) in user_stake.positions.iter().enumerate() {
        if position.is_active && position.pool_id == pool_id {
            position_index = Some(i);
            break;
        }
    }

    let position_idx = position_index.ok_or(ProgramError::Custom(1002))?; 
    let position = &user_stake.positions[position_idx];

    if !position.auto_compound_enabled {
        return Err(ProgramError::Custom(5001)); 
    }

    let current_timestamp = Clock::get()?.unix_timestamp;

    let time_since_last_compound = current_timestamp - position.last_compound_timestamp;
    let required_interval = (position.compound_frequency_hours as i64) * 3600; 

    if time_since_last_compound < required_interval {
        return Err(ProgramError::Custom(5004)); 
    }

    update_pool_rewards(&mut staking_pool, current_timestamp)?;

    let pending_rewards = calculate_position_rewards(
        &user_stake.positions[position_idx],
        &staking_pool,
        current_timestamp
    )?;

    let total_rewards = pending_rewards.saturating_add(position.pending_rewards);

    if total_rewards < position.min_compound_amount {
        return Err(ProgramError::Custom(5002)); 
    }

    if total_rewards == 0 {
        return Err(ProgramError::Custom(3001)); 
    }

    let reward_vault_info = TokenAccount::from_account_info(reward_token_vault)?;
    let user_lst_token_info = TokenAccount::from_account_info(user_lst_token_account)?;

    if reward_vault_info.amount() < total_rewards {
        return Err(ProgramError::Custom(3002)); 
    }

    if *user_lst_token_info.owner() != *position_owner.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    if *user_lst_token_info.mint() != *liquid_stake_mint.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    let protocol_fee = calculate_protocol_fee(total_rewards, global_config.protocol_fee_rate)?;
    let compound_amount = total_rewards.saturating_sub(protocol_fee);

    let global_config_bump_arr = &[global_config_bump];
    let seeds = seeds!(
        b"global_config_account", 
        authority.key().as_ref(),
        global_config_bump_arr
    );
    let signer_seeds = Signer::from(&seeds);

    if protocol_fee > 0 {
        let reward_mint_info = Mint::from_account_info(reward_token_mint)?;
        let signer_seeds_clone = signer_seeds.clone();
        TransferChecked {
            from: reward_token_vault,
            to: treasury_account,
            mint: reward_token_mint,
            authority: global_config_account,
            amount: protocol_fee,
            decimals: reward_mint_info.decimals(),
        }.invoke_signed(&[signer_seeds_clone])?;
    }

    if *reward_token_mint.key() == *stake_token_mint.key() {
        let lst_tokens = compound_amount; 

        let mint_info = Mint::from_account_info(liquid_stake_mint)?;
        
        MintToChecked {
            mint: liquid_stake_mint,
            account: user_lst_token_account,
            mint_authority: global_config_account,
            amount: lst_tokens,
            decimals: mint_info.decimals()
        }.invoke_signed(&[signer_seeds])?;

        user_stake.positions[position_idx].staked_amount = 
            user_stake.positions[position_idx].staked_amount.saturating_add(compound_amount);
        user_stake.positions[position_idx].lst_tokens = 
            user_stake.positions[position_idx].lst_tokens.saturating_add(lst_tokens);

        staking_pool.total_staked = staking_pool.total_staked.saturating_add(compound_amount);
        staking_pool.liquid_stake_supply = staking_pool.liquid_stake_supply.saturating_add(lst_tokens);

    } else {
        return Err(ProgramError::Custom(5005)); 
    }

    user_stake.positions[position_idx].pending_rewards = 0;
    user_stake.positions[position_idx].last_reward_update = current_timestamp;
    user_stake.positions[position_idx].last_compound_timestamp = current_timestamp;
    user_stake.positions[position_idx].compound_count = user_stake.positions[position_idx].compound_count.saturating_add(1);

    user_stake.total_staked_amount = user_stake.total_staked_amount.saturating_add(compound_amount);
    user_stake.total_lst_balance = user_stake.total_lst_balance.saturating_add(compound_amount);
    user_stake.total_earned = user_stake.total_earned.saturating_add(total_rewards);
    user_stake.last_update_timestamp = current_timestamp;

    staking_pool.total_reward_distributed = staking_pool.total_reward_distributed.saturating_add(total_rewards);

    Ok(())
}

fn calculate_position_rewards(
    position: &StakePosition,
    pool: &StakingPool,
    current_timestamp: i64
) -> Result<u64, ProgramError> {
    if position.staked_amount == 0 || !position.is_active {
        return Ok(0);
    }

    let time_elapsed = current_timestamp.saturating_sub(position.last_reward_update);
    
    if time_elapsed <= 0 {
        return Ok(0);
    }

    let user_share = (position.staked_amount as u128)
        .saturating_mul(pool.accumulated_reward_per_share)
        .saturating_div(1_000_000_000_000u128) as u64;

    let time_rewards = (position.staked_amount as u128)
        .saturating_mul(pool.reward_rate_per_second as u128)
        .saturating_mul(time_elapsed as u128)
        .saturating_div(1_000_000_000_000u128) as u64;

    let base_rewards = user_share.saturating_add(time_rewards);

    let final_rewards = if position.lock_exipry_enable && current_timestamp < position.lock_expiry {
        apply_reward_multiplier(base_rewards, pool.reward_multiplier)?
    } else {
        base_rewards
    };

    Ok(final_rewards)
}

fn apply_reward_multiplier(rewards: u64, multiplier: u16) -> Result<u64, ProgramError> {
    if multiplier <= 100 {
        return Ok(rewards);
    }

    let multiplied = (rewards as u128)
        .saturating_mul(multiplier as u128)
        .saturating_div(100u128) as u64;

    Ok(multiplied)
}

fn calculate_protocol_fee(amount: u64, fee_rate: u16) -> Result<u64, ProgramError> {
    let fee = (amount as u128)
        .saturating_mul(fee_rate as u128)
        .saturating_div(10000u128) as u64; 

    Ok(fee)
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
