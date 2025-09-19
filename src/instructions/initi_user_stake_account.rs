use pinocchio::{account_info::AccountInfo, instruction::Signer, program_error::ProgramError, sysvars::{clock::Clock, rent::Rent, Sysvar}, *};
use pinocchio_system::instructions::CreateAccount;
use pinocchio_log::log;

use crate::states::{helper::AccountData, user_stake_account::{StakePosition, UserStakeAccount, MAX_POSITIONS}};

pub fn process_initialize_user_stake_account(accounts: &[AccountInfo]) -> ProgramResult {

    let [user, authority, global_config_account, user_token_account, user_stake_account, _system_program] = accounts else {
        return Err(ProgramError::InvalidAccountData);
    };

    if !user.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    };

    if !user_stake_account.data_is_empty() {
        return Err(ProgramError::AccountAlreadyInitialized);
    };

    if user_token_account.data_is_empty() {
        return Err(ProgramError::UninitializedAccount);
    };

    let (global_config_pda, _bump1) = pubkey::find_program_address(
        &[b"global_config_account", authority.key().as_ref()],
        &crate::ID
    );

    if *global_config_account.key() != global_config_pda {
        return Err(ProgramError::InvalidAccountData);
    };

    let (user_stake_account_pda, user_stake_account_bump) = pubkey::find_program_address(
        &[b"user_stake_account", user.key().as_ref(), global_config_account.key().as_ref()],
        &crate::ID
    );

    if *user_stake_account.key() != user_stake_account_pda {
        return Err(ProgramError::InvalidAccountData);
    };

    let lamports = Rent::get()?.minimum_balance(UserStakeAccount::SIZE);

    let bump_ref = &[user_stake_account_bump];
    let seeds = seeds!(
        b"user_stake_account", 
        user.key().as_ref(), 
        global_config_account.key().as_ref(),
        bump_ref
    );
    let signer_seeds = Signer::from(&seeds);

    CreateAccount {
        from: user,
        to: user_stake_account,
        lamports,
        space: UserStakeAccount::SIZE as u64,
        owner: &crate::ID,
    }.invoke_signed(&[signer_seeds])?;

    let mut user_stake_account_info = UserStakeAccount::from_account_info_mut(user_stake_account)?;

    user_stake_account_info.owner = *user.key();
    user_stake_account_info.global_config = *global_config_account.key();
    user_stake_account_info.user_token_account = *user_token_account.key();
    user_stake_account_info.total_lst_balance = 0;
    user_stake_account_info.total_staked_amount = 0;
    user_stake_account_info.total_pending_rewards = 0;
    user_stake_account_info.active_positions = 0;
    user_stake_account_info.creation_timestamp = Clock::get()?.unix_timestamp;
    user_stake_account_info.is_paused = false;
    user_stake_account_info.positions = [StakePosition::default(); MAX_POSITIONS];
    user_stake_account_info.bump = user_stake_account_bump;

    log!("User Stake Account Initialized Successfully!");
    log!("Owner: {}", &user_stake_account_info.owner);
    log!("Total LST Balance: {}", user_stake_account_info.total_lst_balance);
    log!("Total Staked Amount: {}", user_stake_account_info.total_staked_amount);
    log!("Active Positions: {}", user_stake_account_info.active_positions);
    log!("Creation Timestamp: {}", user_stake_account_info.creation_timestamp);
    log!("Active Positions: {}", user_stake_account_info.active_positions);

    Ok(())
}