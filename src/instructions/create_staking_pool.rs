use pinocchio::{account_info::AccountInfo, instruction::Signer, program_error::ProgramError, pubkey::Pubkey, sysvars::{clock::Clock, rent::Rent, Sysvar}, *};
use pinocchio_system::instructions::CreateAccount;
use pinocchio_token::{instructions::{InitializeAccount3, InitializeMint2}, state::{Mint, TokenAccount}};
use crate::states::{global_config::GlobalConfig, helper::AccountData, staking_pool_account::{PoolStatusEnum, SlashTypeEnum, StakingPool}};

const MAX_POOLS: usize = 10;

pub fn process_create_staking_pool(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    let [
        authority, 
        creator,
        stake_token_mint, 
        reward_token_mint, 
        stake_token_vault,
        reward_token_vault,
        staking_pool_account, 
        global_config_account, 
        liquid_stake_mint, 
        price_feed_account,
        system_program,
        token_program,
    ] = accounts else {
        return Err(ProgramError::InvalidAccountData);
    };

    if !creator.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    };

    if instruction_data.len() < 64 { 
        return Err(ProgramError::InvalidInstructionData);
    };

    let pool_id = u64::from_le_bytes(
        instruction_data[0..8].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );

    let reward_rate_per_second = u64::from_le_bytes( 
        instruction_data[8..16].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );

    let lock_period_enabled = instruction_data[16]; 

    let lock_period_duration = i64::from_le_bytes(
        instruction_data[17..25].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );

    let reward_multiplier = u16::from_le_bytes(
        instruction_data[25..27].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );

    let early_withdraw_penalty = u64::from_le_bytes( 
        instruction_data[27..35].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );

    let slashing_enabled = instruction_data[35]; 

    let slashing_condition_type = instruction_data[36]; 

    let slash_percentage = u16::from_le_bytes(
        instruction_data[37..39].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );

    let min_evidence_required = instruction_data[39]; 

    let cooldown_period = i64::from_le_bytes( 
        instruction_data[40..48].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );

    let maximum_stake_limit = u64::from_le_bytes(
        instruction_data[48..56].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );

    let minimum_stake_amount = u64::from_le_bytes(
        instruction_data[56..64].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );

    if reward_multiplier == 0 {
        return Err(ProgramError::InvalidInstructionData);
    };
    
    if slash_percentage > 10000 { 
        return Err(ProgramError::InvalidInstructionData);
    };

    if minimum_stake_amount == 0 {
        return Err(ProgramError::InvalidInstructionData);
    };

    if maximum_stake_limit != 0 && maximum_stake_limit < minimum_stake_amount {
        return Err(ProgramError::InvalidInstructionData);
    };

    let clock = Clock::get()?;

    let (staking_pool_pda, staking_pool_bump) = pubkey::find_program_address(
        &[
            b"staking_pool",
            creator.key().as_ref(),
            pool_id.to_le_bytes().as_ref(),
        ],
        &crate::ID, // Your program ID
    );

    if *staking_pool_account.key() != staking_pool_pda {
        return Err(ProgramError::InvalidAccountData);
    };

    let (stake_token_vault_pda, stake_token_vault_bump) = pubkey::find_program_address(
        &[
            b"stake_token_vault",
            stake_token_mint.key().as_ref(),
            global_config_account.key().as_ref()
        ],
        &crate::ID
    );

    if *stake_token_vault.key() != stake_token_vault_pda {
        return Err(ProgramError::InvalidAccountData);
    };

    let (reward_token_vault_pda, reward_token_vault_bump) = pubkey::find_program_address(
        &[
            b"reward_token_vault",
            reward_token_mint.key().as_ref(),
            global_config_account.key().as_ref()
        ],
        &crate::ID
    );

    if *reward_token_vault.key() != reward_token_vault_pda {
        return Err(ProgramError::InvalidAccountData);
    };

    let (liquid_stake_mint_pda, liquid_stake_mint_bump) = pubkey::find_program_address(
        &[b"liquid_stake_mint", creator.key().as_ref()],
        &crate::ID
    );

    if *liquid_stake_mint.key() != liquid_stake_mint_pda {
        return Err(ProgramError::InvalidAccountData);
    };

    if stake_token_vault.data_is_empty() {
        let lamports = Rent::get()?.minimum_balance(TokenAccount::LEN);

        let bump_ref = &[stake_token_vault_bump];
        let seeds = seeds!(
            b"stake_token_vault",
            stake_token_mint.key().as_ref(),
            global_config_account.key().as_ref(),
            bump_ref
        );
        let signer_seeds = Signer::from(&seeds);

        CreateAccount {
            from: creator,
            to: stake_token_vault,
            lamports,
            space: TokenAccount::LEN as u64,
            owner: &pinocchio_token::ID
        }.invoke_signed(&[signer_seeds])?;

        InitializeAccount3 {
            account: stake_token_vault,
            mint: stake_token_mint,
            owner: &global_config_account.key()
        }.invoke()?;
    };

    if reward_token_vault.data_is_empty() {
        let lamports = Rent::get()?.minimum_balance(TokenAccount::LEN);

        let bump_ref = &[reward_token_vault_bump];
        let seeds = seeds!(
            b"reward_token_vault",
            reward_token_mint.key().as_ref(),
            global_config_account.key().as_ref(),
            bump_ref
        );
        let signer_seeds = Signer::from(&seeds);

        CreateAccount {
            from: creator,
            to: reward_token_vault,
            lamports,
            space: TokenAccount::LEN as u64,
            owner: &pinocchio_token::ID
        }.invoke_signed(&[signer_seeds])?;

        InitializeAccount3 {
            account: reward_token_vault,
            mint: reward_token_mint,
            owner: &global_config_account.key()
        }.invoke()?;
    };

    if liquid_stake_mint.data_is_empty() {
        let lamports = Rent::get()?.minimum_balance(Mint::LEN);

        let bump_ref = &[liquid_stake_mint_bump];
        let seeds = seeds!(
            b"liquid_stake_mint", 
            creator.key().as_ref(),
            bump_ref
        );
        let signer_seeds = Signer::from(&seeds);

        CreateAccount {
            from: creator,
            to: liquid_stake_mint,
            lamports,
            space: Mint::LEN as u64,
            owner: &pinocchio_token::ID 
        }.invoke_signed(&[signer_seeds])?;

        InitializeMint2 {
            mint: liquid_stake_mint,
            decimals: 9,
            mint_authority: &global_config_account.key(),
            freeze_authority: Some(&global_config_account.key()),
        }.invoke()?;
    };

    if staking_pool_account.data_is_empty() {
        let lamports = Rent::get()?.minimum_balance(StakingPool::SIZE);

        let bump_ref = &[staking_pool_bump];
        let pool_id_ref = pool_id.to_le_bytes();
        let seeds = seeds!(
            b"staking_pool",
            creator.key().as_ref(),
            pool_id_ref.as_ref(),
            bump_ref
        );
        let signer_seeds = Signer::from(&seeds);

        CreateAccount {
            from: creator,
            to: staking_pool_account,
            lamports,
            space: StakingPool::SIZE as u64,
            owner: &crate::ID
        }.invoke_signed(&[signer_seeds])?;

        let lock_period_enabled_bool = lock_period_enabled != 0;
        let slashing_enabled_bool = slashing_enabled != 0;

        let mut staking_pool_account_info = StakingPool::from_account_info_mut(staking_pool_account)?;
        staking_pool_account_info.authority = *authority.key();
        staking_pool_account_info.pool_id = pool_id;
        staking_pool_account_info.creation_timestamp = clock.unix_timestamp;
        staking_pool_account_info.pool_status = 0;
        staking_pool_account_info.stake_token_mint = *stake_token_mint.key();
        staking_pool_account_info.reward_token_mint = *reward_token_mint.key();
        staking_pool_account_info.stake_token_vault = *stake_token_vault.key();
        staking_pool_account_info.reward_token_vault = *reward_token_vault.key();
        staking_pool_account_info.total_staked = 0;
        staking_pool_account_info.total_reward_distributed = 0;
        staking_pool_account_info.reward_rate_per_second = reward_rate_per_second;
        staking_pool_account_info.accumulated_reward_per_share = 0;
        staking_pool_account_info.lock_period_enabled = lock_period_enabled_bool;
        staking_pool_account_info.lock_period_duration = lock_period_duration;
        staking_pool_account_info.reward_multiplier = reward_multiplier;
        staking_pool_account_info.early_withdraw_penalty = early_withdraw_penalty;
        staking_pool_account_info.slashing_enabled = slashing_enabled_bool;
        staking_pool_account_info.slashing_condition_type = slashing_condition_type;
        staking_pool_account_info.slash_percentage = slash_percentage;
        staking_pool_account_info.min_evidence_required = min_evidence_required;
        staking_pool_account_info.cooldown_period = cooldown_period;
        staking_pool_account_info.price_feed_account = *price_feed_account.key();
        staking_pool_account_info.maximum_stake_limit = maximum_stake_limit;
        staking_pool_account_info.minimum_stake_amount = minimum_stake_amount;
        staking_pool_account_info.liquid_stake_mint = *liquid_stake_mint.key();
        staking_pool_account_info.liquid_stake_supply = 0;
        staking_pool_account_info.emergency_pause_flag = false;
        staking_pool_account_info.stake_pool_bump = staking_pool_bump;
    }

    let mut global_config_account_info = GlobalConfig::from_account_info_mut(global_config_account)?;
    global_config_account_info.total_pools_created = global_config_account_info
        .total_pools_created
        .checked_add(1)
        .ok_or(ProgramError::ArithmeticOverflow)?;

    global_config_account_info.active_pools = global_config_account_info
        .active_pools
        .checked_add(1)
        .ok_or(ProgramError::ArithmeticOverflow)?;

    if global_config_account_info.active_pool_keys.len() >= MAX_POOLS {
        return Err(ProgramError::AccountDataTooSmall);
    }
    global_config_account_info.active_pool_keys.push(*staking_pool_account.key());

    Ok(())
}