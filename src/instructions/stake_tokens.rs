use pinocchio::{account_info::AccountInfo, instruction::Signer, program_error::ProgramError, sysvars::{clock::Clock, Sysvar}, *};
use pinocchio_token::{instructions::{TransferChecked, MintToChecked}, state::{Mint, TokenAccount}};

use crate::states::{
    global_config::GlobalConfig, 
    helper::AccountData, 
    staking_pool_account::StakingPool, 
    user_stake_account::{StakePosition, UserStakeAccount}
};

pub fn process_stake_tokens(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    let [
        user,                    // Staker (signer)
        authority,              // Global config authority  
        creator,                // Pool creator
        stake_token_mint,       // Token being staked
        stake_token_vault,      // Where staked tokens go
        liquid_stake_mint,      // LST mint
        global_config_account,  // Global config PDA
        staking_pool_account,   // Pool account
        user_token_account,     // User's source token account
        user_stake_account,     // User's stake position account
        user_lst_token_account, // User's LST token account
        token_program,          // Token program
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

    let (global_config_pda, global_config_bump) = pubkey::find_program_address(
        &[b"global_config_account", authority.key().as_ref()],
        &crate::ID
    );

    if *global_config_account.key() != global_config_pda {
        return Err(ProgramError::InvalidAccountData);
    }

    let (staking_pool_pda, _staking_pool_bump) = pubkey::find_program_address(
        &[
            b"staking_pool",
            creator.key().as_ref(),
            pool_id.to_le_bytes().as_ref(),
        ],
        &crate::ID,
    );

    if *staking_pool_account.key() != staking_pool_pda {
        return Err(ProgramError::InvalidAccountData);
    }

    let (stake_token_vault_pda, _stake_token_vault_bump) = pubkey::find_program_address(
        &[
            b"stake_token_vault",
            stake_token_mint.key().as_ref(),
            global_config_account.key().as_ref()
        ],
        &crate::ID
    );

    if *stake_token_vault.key() != stake_token_vault_pda {
        return Err(ProgramError::InvalidAccountData);
    }

    let (liquid_stake_mint_pda, _liquid_stake_mint_bump) = pubkey::find_program_address(
        &[b"liquid_stake_mint", creator.key().as_ref()],
        &crate::ID
    );

    if *liquid_stake_mint.key() != liquid_stake_mint_pda {
        return Err(ProgramError::InvalidAccountData);
    }

    // Verify user stake account PDA
    let (user_stake_pda, _user_stake_bump) = pubkey::find_program_address(
        &[
            b"user_stake_account",
            user.key().as_ref(),
            global_config_account.key().as_ref(),
        ],
        &crate::ID
    );

    if *user_stake_account.key() != user_stake_pda {
        return Err(ProgramError::InvalidAccountData);
    }

    let global_config = GlobalConfig::from_account_info(global_config_account)?;
    let mut staking_pool = StakingPool::from_account_info_mut(staking_pool_account)?;
    let mut user_stake = UserStakeAccount::from_account_info_mut(user_stake_account)?;

    if staking_pool.pool_status != 0 { // Not active
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

    if stake_amount < global_config.min_stake_amount {
        return Err(ProgramError::InvalidArgument);
    }

    if staking_pool.maximum_stake_limit > 0 && 
       staking_pool.total_staked.saturating_add(stake_amount) > staking_pool.maximum_stake_limit {
        return Err(ProgramError::InvalidArgument);
    }

    let user_token_info = TokenAccount::from_account_info(user_token_account)?;
    let stake_vault_info = TokenAccount::from_account_info(stake_token_vault)?;
    let user_lst_token_info = TokenAccount::from_account_info(user_lst_token_account)?;

    if *user_token_info.owner() != *user.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    if *user_token_info.mint() != *stake_token_mint.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    if user_token_info.amount() < stake_amount {
        return Err(ProgramError::InsufficientFunds);
    }

    if *stake_vault_info.owner() != *global_config_account.key() {
        return Err(ProgramError::InvalidAccountData);
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

    let mut position_index = None;
    for (i, position) in user_stake.positions.iter().enumerate() {
        if !position.is_active {
            position_index = Some(i);
            break;
        }
    }

    let position_idx = position_index.ok_or(ProgramError::AccountDataTooSmall)?;

    let current_timestamp = Clock::get()?.unix_timestamp;
    update_pool_rewards(&mut staking_pool, current_timestamp)?;

    let lst_tokens = stake_amount;

    let stake_mint_info = Mint::from_account_info(stake_token_mint)?;

    TransferChecked {
        from: user_token_account,
        to: stake_token_vault,
        mint: stake_token_mint,
        authority: user,
        amount: stake_amount,
        decimals: stake_mint_info.decimals(),
    }.invoke()?;

    let global_config_bump = &[global_config_bump];
    let seeds = seeds!(
        b"global_config_account", 
        authority.key().as_ref(),
        global_config_bump
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

    user_stake.positions[position_idx] = StakePosition {
        pool_id,
        staking_pool: *staking_pool_account.key(),
        lst_token_account: *user_lst_token_account.key(),
        staked_amount: stake_amount,
        lst_tokens,
        last_reward_update: current_timestamp,
        pending_rewards: 0,
        stake_timestamp: current_timestamp,
        lock_exipry_enable: staking_pool.lock_period_enabled,
        lock_expiry: if staking_pool.lock_period_enabled {
            current_timestamp.saturating_add(staking_pool.lock_period_duration)
        } else {
            0
        },
        is_active: true,
        bump: 0,
    };

    user_stake.total_staked_amount = user_stake.total_staked_amount.saturating_add(stake_amount);
    user_stake.total_lst_balance = user_stake.total_lst_balance.saturating_add(lst_tokens);
    user_stake.active_positions = user_stake.active_positions.saturating_add(1);
    user_stake.last_update_timestamp = current_timestamp;

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