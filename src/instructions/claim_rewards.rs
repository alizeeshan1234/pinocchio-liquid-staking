use pinocchio::{account_info::AccountInfo, program_error::ProgramError, instruction::Signer, sysvars::{clock::Clock, Sysvar}, *};
use pinocchio_token::{state::{TokenAccount, Mint}, instructions::TransferChecked};

use crate::states::{
    helper::AccountData, 
    staking_pool_account::StakingPool, 
    user_stake_account::{UserStakeAccount, ClaimEvent, MAX_HISTORY},
    global_config::GlobalConfig
};

pub fn process_claim_rewards(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    let [
        user,                      // User claiming rewards
        authority,                 // Global config authority for PDA signing
        reward_token_mint,         // Reward token mint
        reward_token_vault,        // Pool's reward vault
        global_config_account,     // Global config PDA
        staking_pool_account,      // Pool account
        user_reward_token_account, // User's reward token account
        user_stake_account,        // User's stake position account
        treasury_account,          // Treasury for protocol fees
        token_program,             // Token program
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

    // Verify PDAs
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

    // Validate user owns the stake account
    if user_stake.owner != *user.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    if user_stake.is_paused {
        return Err(ProgramError::Custom(4001)); // User account is paused
    }

    if global_config.emergency_pause {
        return Err(ProgramError::Custom(4002)); // Global emergency pause
    }

    // Find user's position for this pool
    let mut position_index = None;
    for (i, position) in user_stake.positions.iter().enumerate() {
        if position.is_active && position.pool_id == pool_id {
            position_index = Some(i);
            break;
        }
    }

    let position_idx = position_index.ok_or(ProgramError::Custom(1002))?; // Position not found

    let current_timestamp = Clock::get()?.unix_timestamp;

    // Update pool rewards first
    update_pool_rewards(&mut staking_pool, current_timestamp)?;

    // Calculate pending rewards for this specific position
    let pending_rewards = calculate_position_rewards(
        &mut user_stake.positions[position_idx],
        &staking_pool,
        current_timestamp
    )?;

    // Add any accumulated pending rewards
    let total_claimable = pending_rewards.saturating_add(user_stake.positions[position_idx].pending_rewards);

    if total_claimable == 0 {
        return Err(ProgramError::Custom(3001)); // No rewards to claim
    }

    // Validate accounts
    let user_reward_token_info = TokenAccount::from_account_info(user_reward_token_account)?;
    let reward_vault_info = TokenAccount::from_account_info(reward_token_vault)?;
    let treasury_info = TokenAccount::from_account_info(treasury_account)?;

    if *user_reward_token_info.owner() != *user.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    if *user_reward_token_info.mint() != *reward_token_mint.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    if *reward_vault_info.mint() != staking_pool.reward_token_mint {
        return Err(ProgramError::InvalidAccountData);
    }

    // Check if reward vault has sufficient balance
    if reward_vault_info.amount() < total_claimable {
        return Err(ProgramError::Custom(3002)); // Insufficient rewards in vault
    }

    // Calculate protocol fee
    let protocol_fee = calculate_protocol_fee(total_claimable, global_config.protocol_fee_rate)?;
    let user_rewards = total_claimable.saturating_sub(protocol_fee);

    let global_config_bump_arr = &[global_config_bump];
    let seeds = seeds!(
        b"global_config_account", 
        authority.key().as_ref(),
        global_config_bump_arr
    );
    let signer_seeds = Signer::from(&seeds);

    let reward_mint_info = Mint::from_account_info(reward_token_mint)?;

    let signer_seeds_clone = signer_seeds.clone();
    if user_rewards > 0 {
        TransferChecked {
            from: reward_token_vault,
            to: user_reward_token_account,
            mint: reward_token_mint,
            authority: global_config_account,
            amount: user_rewards,
            decimals: reward_mint_info.decimals(),
        }.invoke_signed(&[signer_seeds_clone])?;
    }

    if protocol_fee > 0 {
        TransferChecked {
            from: reward_token_vault,
            to: treasury_account,
            mint: reward_token_mint,
            authority: global_config_account,
            amount: protocol_fee,
            decimals: reward_mint_info.decimals(),
        }.invoke_signed(&[signer_seeds])?;
    }

    user_stake.positions[position_idx].pending_rewards = 0;
    user_stake.positions[position_idx].last_reward_update = current_timestamp;

    user_stake.total_earned = user_stake.total_earned.saturating_add(total_claimable);
    user_stake.total_claimed = user_stake.total_claimed.saturating_add(user_rewards);
    user_stake.pending_rewards = user_stake.pending_rewards.saturating_sub(total_claimable);
    user_stake.last_claim_timestamp = current_timestamp;
    user_stake.last_update_timestamp = current_timestamp;

    add_claim_to_history(&mut user_stake, user_rewards, current_timestamp)?;

    staking_pool.total_reward_distributed = staking_pool.total_reward_distributed.saturating_add(total_claimable);

    Ok(())
}

pub fn process_claim_all_rewards(accounts: &[AccountInfo], _instruction_data: &[u8]) -> ProgramResult {
    let [
        user,
        authority,
        reward_token_mint,
        reward_token_vault,
        global_config_account,
        user_reward_token_account,
        user_stake_account,
        treasury_account,
        token_program,
    ] = accounts else {
        return Err(ProgramError::InvalidAccountData);
    };

    if !user.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let (global_config_pda, global_config_bump) = pubkey::find_program_address(
        &[b"global_config_account", authority.key().as_ref()],
        &crate::ID
    );

    if *global_config_account.key() != global_config_pda {
        return Err(ProgramError::InvalidAccountData);
    }

    let global_config = GlobalConfig::from_account_info(global_config_account)?;
    let mut user_stake = UserStakeAccount::from_account_info_mut(user_stake_account)?;

    if user_stake.owner != *user.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    if user_stake.is_paused || global_config.emergency_pause {
        return Err(ProgramError::Custom(4002));
    }

    let current_timestamp = Clock::get()?.unix_timestamp;
    let mut total_claimable = 0u64;

    for position in user_stake.positions.iter_mut() {
        if position.is_active && position.staked_amount > 0 {
            let position_rewards = position.pending_rewards;
            total_claimable = total_claimable.saturating_add(position_rewards);
            
            position.pending_rewards = 0;
            position.last_reward_update = current_timestamp;
        }
    }

    total_claimable = total_claimable.saturating_add(user_stake.pending_rewards);

    if total_claimable == 0 {
        return Err(ProgramError::Custom(3001));
    }

    let protocol_fee = calculate_protocol_fee(total_claimable, global_config.protocol_fee_rate)?;
    let user_rewards = total_claimable.saturating_sub(protocol_fee);

    let global_config_bump_arr = &[global_config_bump];
    let seeds = seeds!(
        b"global_config_account", 
        authority.key().as_ref(),
        global_config_bump_arr
    );
    let signer_seeds = Signer::from(&seeds);

    let reward_mint_info = Mint::from_account_info(reward_token_mint)?;

    if user_rewards > 0 {
        TransferChecked {
            from: reward_token_vault,
            to: user_reward_token_account,
            mint: reward_token_mint,
            authority: global_config_account,
            amount: user_rewards,
            decimals: reward_mint_info.decimals(),
        }.invoke_signed(&[signer_seeds])?;
    }

    user_stake.total_earned = user_stake.total_earned.saturating_add(total_claimable);
    user_stake.total_claimed = user_stake.total_claimed.saturating_add(user_rewards);
    user_stake.pending_rewards = 0;
    user_stake.last_claim_timestamp = current_timestamp;
    user_stake.last_update_timestamp = current_timestamp;

    add_claim_to_history(&mut user_stake, user_rewards, current_timestamp)?;

    Ok(())
}

fn calculate_position_rewards(
    position: &mut crate::states::user_stake_account::StakePosition,
    pool: &StakingPool,
    current_timestamp: i64
) -> Result<u64, ProgramError> {
    if position.staked_amount == 0 || !position.is_active {
        return Ok(0);
    }

    let time_elapsed = current_timestamp.saturating_sub(position.last_reward_update);
    if time_elapsed == 0 {
        return Ok(0);
    };

    if time_elapsed <= 0 {
        return Ok(0);
    }

    let base_rewards = (position.staked_amount as u128)
        .saturating_mul(pool.reward_rate_per_second as u128)
        .saturating_mul(time_elapsed as u128)
        .saturating_div(1_000_000_000_000u128) as u64;

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

fn add_claim_to_history(
    user_stake: &mut UserStakeAccount, 
    amount: u64, 
    timestamp: i64
) -> Result<(), ProgramError> {
    for i in 1..MAX_HISTORY {
        user_stake.claim_history[i - 1] = user_stake.claim_history[i];
    }
    
    user_stake.claim_history[MAX_HISTORY - 1] = ClaimEvent {
        amount,
        timestamp,
    };

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