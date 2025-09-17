use pinocchio::{account_info::AccountInfo, instruction::Signer, program_error::ProgramError, pubkey::Pubkey, sysvars::{clock::Clock, rent::Rent, Sysvar}, *};
use pinocchio_system::instructions::CreateAccount;
use pinocchio_log::log;
use crate::states::{oracle_config::OracleConfigInfo, helper::AccountData};

pub fn process_init_oracle_config(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {

    let [oracle_authority, oracle_config_account, price_feed_account, system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !oracle_authority.is_signer() {
        return Err(ProgramError::InvalidAccountData);
    };

    let update_frequency_seconds = i64::from_le_bytes(
        instruction_data[0..8].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );

    let current_price = u64::from_le_bytes(
        instruction_data[8..16].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );

    let (oracle_config_pda, bump) = pubkey::find_program_address(
        &[b"oracle_config_account", oracle_authority.key().as_ref()],
        &crate::ID
    );

    if *oracle_config_account.key() != oracle_config_pda {
        return Err(ProgramError::InvalidAccountData);
    };

    if oracle_config_account.data_is_empty() {
        msg!("Initializing Oracle Account");

        let lamports = Rent::get()?.minimum_balance(OracleConfigInfo::SIZE);

        let bump_ref = &[bump];
        let seeds = seeds!(
            b"oracle_config_account", 
            oracle_authority.key().as_ref(),
            bump_ref
        );
        let signer_seeds = Signer::from(&seeds);

        CreateAccount {
            from: oracle_authority,
            to: oracle_config_account,
            lamports,
            space: OracleConfigInfo::SIZE as u64,
            owner: &crate::ID
        }.invoke_signed(&[signer_seeds])?;

        let mut oracle_config_account_info_mut = OracleConfigInfo::from_account_info_mut(oracle_config_account)?;
        oracle_config_account_info_mut.price_feed_account = *price_feed_account.key();
        oracle_config_account_info_mut.update_frequency_seconds = update_frequency_seconds;
        oracle_config_account_info_mut.oracle_authority = *oracle_authority.key();
        oracle_config_account_info_mut.last_update_timestamp = Clock::get()?.unix_timestamp;
        oracle_config_account_info_mut.current_price = current_price;
        oracle_config_account_info_mut.oracle_account_bump = bump;
    } else {
        return Err(ProgramError::AccountAlreadyInitialized);
    };

    Ok(())
}

pub fn process_update_price(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {

    let [oracle_authority, oracle_config_account] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !oracle_authority.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    };

    let new_price = u64::from_le_bytes(
        instruction_data[0..8].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );

    let (oracle_config_pda, _bump) = pubkey::find_program_address(
        &[b"oracle_config_account", oracle_authority.key().as_ref()],
        &crate::ID
    );

    if *oracle_config_account.key() != oracle_config_pda {
        return Err(ProgramError::InvalidAccountData);
    };

    let mut oracle_account_info = OracleConfigInfo::from_account_info_mut(oracle_config_account)?;

    if oracle_account_info.oracle_authority != *oracle_authority.key() {
        return Err(ProgramError::InvalidAccountOwner);
    };
    let current_timestamp = Clock::get()?.unix_timestamp;
    
    oracle_account_info.current_price = new_price;
    oracle_account_info.last_update_timestamp = current_timestamp;

    Ok(())
}

pub fn get_oracle_price(accounts: &[AccountInfo]) -> ProgramResult {

    let [oracle_authority, oracle_config_account] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);   
    };

    let (oracle_config_pda, _bump) = pubkey::find_program_address(
        &[b"oracle_config_account", oracle_authority.key().as_ref()],
        &crate::ID
    );

    if *oracle_config_account.key() != oracle_config_pda {
        return Err(ProgramError::InvalidAccountData);
    };

    let oracle_account_info = OracleConfigInfo::from_account_info(oracle_config_account)?;

    let current_price = oracle_account_info.current_price;

    log!("Current Price: {}", current_price);

    Ok(())
}