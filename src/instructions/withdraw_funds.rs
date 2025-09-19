use pinocchio::{account_info::AccountInfo, program_error::ProgramError, instruction::Signer, sysvars::{clock::Clock, Sysvar}, *};
use pinocchio_token::{state::{TokenAccount, Mint}, instructions::{TransferChecked, BurnChecked}};

use crate::states::{helper::AccountData, staking_pool_account::StakingPool, user_stake_account::UserStakeAccount, global_config::GlobalConfig};

pub fn process_emergency_withdraw(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    let [
        user,                    // User requesting emergency withdrawal
        authority,              // Global config authority (for emergency validation)
        stake_token_mint,       // Original staked token mint
        stake_token_vault,      // Vault holding staked tokens
        liquid_stake_mint,      // LST mint (to burn tokens)
        global_config_account,  // Global config PDA
        staking_pool_account,   // Pool account
        user_token_account,     // User's token account to receive tokens
        user_stake_account,     // User's stake position account
        user_lst_token_account, // User's LST token account
        treasury_account,       // Treasury account for penalty collection
        token_program,          // Token program
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

    // Emergency withdraw conditions - at least one must be true
    let emergency_conditions_met = check_emergency_conditions(
        &global_config,
        &staking_pool,
        pool_id
    )?;

    if !emergency_conditions_met {
        return Err(ProgramError::Custom(2001)); // No emergency conditions met
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
    let position = &user_stake.positions[position_idx];

    if position.lst_tokens == 0 {
        return Err(ProgramError::Custom(2002)); // No tokens to withdraw
    }

    let current_timestamp = Clock::get()?.unix_timestamp;

    // Calculate emergency withdrawal amounts
    let lst_amount = position.lst_tokens;
    let underlying_tokens = position.staked_amount;

    // Calculate emergency penalty (usually higher than normal early withdrawal)
    let emergency_penalty = calculate_emergency_penalty(
        underlying_tokens,
        &staking_pool,
        &global_config
    )?;

    let tokens_after_penalty = underlying_tokens.saturating_sub(emergency_penalty);

    // Validate accounts
    let user_token_info = TokenAccount::from_account_info(user_token_account)?;
    let user_lst_token_info = TokenAccount::from_account_info(user_lst_token_account)?;
    let treasury_info = TokenAccount::from_account_info(treasury_account)?;

    if *user_token_info.owner() != *user.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    if *user_lst_token_info.owner() != *user.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    if user_lst_token_info.amount() < lst_amount {
        return Err(ProgramError::InsufficientFunds);
    }

    // Burn all LST tokens for this position
    let lst_mint_info = Mint::from_account_info(liquid_stake_mint)?;
    
    BurnChecked {
        mint: liquid_stake_mint,
        account: user_lst_token_account,
        authority: user,
        amount: lst_amount,
        decimals: lst_mint_info.decimals(),
    }.invoke()?;

    // Transfer penalty to treasury if applicable
    let global_config_bump_arr = &[global_config_bump];
    let seeds = seeds!(
        b"global_config_account", 
        authority.key().as_ref(),
        global_config_bump_arr
    );
    let signer_seeds = Signer::from(&seeds);

    let stake_mint_info = Mint::from_account_info(stake_token_mint)?;

    let signer_seeds_clone = signer_seeds.clone();
    if emergency_penalty > 0 {
        TransferChecked {
            from: stake_token_vault,
            to: treasury_account,
            mint: stake_token_mint,
            authority: global_config_account,
            amount: emergency_penalty,
            decimals: stake_mint_info.decimals(),
        }.invoke_signed(&[signer_seeds_clone])?;
    }

    // Transfer remaining tokens to user
    if tokens_after_penalty > 0 {
        TransferChecked {
            from: stake_token_vault,
            to: user_token_account,
            mint: stake_token_mint,
            authority: global_config_account,
            amount: tokens_after_penalty,
            decimals: stake_mint_info.decimals(),
        }.invoke_signed(&[signer_seeds])?;
    }

    // Deactivate the position completely (emergency withdraw = full exit)
    user_stake.positions[position_idx].staked_amount = 0;
    user_stake.positions[position_idx].lst_tokens = 0;
    user_stake.positions[position_idx].is_active = false;
    user_stake.positions[position_idx].last_reward_update = current_timestamp;

    // Update user totals
    user_stake.total_staked_amount = user_stake.total_staked_amount.saturating_sub(underlying_tokens);
    user_stake.total_lst_balance = user_stake.total_lst_balance.saturating_sub(lst_amount);
    user_stake.active_positions = user_stake.active_positions.saturating_sub(1);
    user_stake.last_update_timestamp = current_timestamp;

    // Update pool totals
    staking_pool.total_staked = staking_pool.total_staked.saturating_sub(underlying_tokens);
    staking_pool.liquid_stake_supply = staking_pool.liquid_stake_supply.saturating_sub(lst_amount);

    // Log emergency withdrawal event (you might want to emit an event here)
    
    Ok(())
}

// Check if emergency conditions are met
fn check_emergency_conditions(
    global_config: &GlobalConfig,
    staking_pool: &StakingPool,
    _pool_id: u64
) -> Result<bool, ProgramError> {
    // Global emergency pause
    if global_config.emergency_pause {
        return Ok(true);
    }

    // Pool-specific emergency conditions
    if staking_pool.emergency_pause_flag {
        return Ok(true);
    }

    // Pool is deprecated
    if staking_pool.pool_status == 2 { // Assuming 2 = deprecated
        return Ok(true);
    }

    // Protocol-level slashing event detected
    if staking_pool.slashing_enabled && has_slashing_event(staking_pool)? {
        return Ok(true);
    }

    // Smart contract vulnerability detected (you'd implement this logic)
    if detect_vulnerability()? {
        return Ok(true);
    }

    // No emergency conditions met
    Ok(false)
}

// Calculate emergency withdrawal penalty
fn calculate_emergency_penalty(
    amount: u64,
    pool: &StakingPool,
    global_config: &GlobalConfig
) -> Result<u64, ProgramError> {
    // Emergency penalty is typically higher than normal early withdrawal
    // You might have different penalty rates for different emergency types
    
    let base_penalty_rate = pool.early_withdraw_penalty; // basis points
    let emergency_multiplier = 150; // 1.5x normal penalty
    
    let emergency_penalty_rate = base_penalty_rate.saturating_mul(emergency_multiplier).saturating_div(100);
    
    // Cap at maximum penalty (e.g., 50% of staked amount)
    let max_penalty_rate = 5000u64; // 50% in basis points
    let final_penalty_rate = emergency_penalty_rate.min(max_penalty_rate);
    
    let penalty = (amount as u128)
        .saturating_mul(final_penalty_rate as u128)
        .saturating_div(10000u128) as u64;
    
    Ok(penalty)
}

// Check for slashing events
fn has_slashing_event(pool: &StakingPool) -> Result<bool, ProgramError> {
    // Implement your slashing detection logic here
    // This might check various conditions like:
    // - Validator misbehavior
    // - Oracle price manipulation
    // - Protocol violations
    
    // For now, return false - implement based on your slashing conditions
    Ok(false)
}

// Detect smart contract vulnerabilities
fn detect_vulnerability() -> Result<bool, ProgramError> {
    // Implement vulnerability detection logic
    // This might include:
    // - Checking for abnormal pool states
    // - Validating critical invariants
    // - Monitoring for exploitation patterns
    
    // For now, return false - implement based on your security requirements
    Ok(false)
}