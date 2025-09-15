use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey, *};
use crate::states::{global_config::GlobalConfig, helper::AccountData};

pub fn process_update_authority(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    let [authority, global_config_account] = accounts else {
        return Err(ProgramError::InvalidAccountData);
    };

    if !authority.is_signer() {
        return Err(ProgramError::InvalidAccountData);
    };

    if instruction_data.len() < 32 {
        return Err(ProgramError::InvalidInstructionData);
    };

    let new_authority = Pubkey::try_from(&instruction_data[0..32])
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    let (global_config_pda, _bump) = pubkey::find_program_address(
        &[b"global_config_account", authority.key().as_ref()],
        &crate::ID
    );

    if *global_config_account.key() != global_config_pda {
        return Err(ProgramError::InvalidAccountData);
    };

    let mut global_config_info = GlobalConfig::from_account_info_mut(global_config_account)?;
    
    if global_config_info.authority != *authority.key() {
        return Err(ProgramError::InvalidAccountData);
    };

    global_config_info.authority = new_authority;
    Ok(())
}

pub fn process_update_protocol_fee(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    let [authority, global_config_account] = accounts else {
        return Err(ProgramError::InvalidAccountData);
    };

    if !authority.is_signer() {
        return Err(ProgramError::InvalidAccountData);
    };

    if instruction_data.len() < 2 {
        return Err(ProgramError::InvalidInstructionData);
    };

    let new_protocol_fee_rate = u16::from_le_bytes(
        instruction_data[0..2].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );

    if new_protocol_fee_rate > 10000 {
        return Err(ProgramError::InvalidArgument);
    };

    let (global_config_pda, _bump) = pubkey::find_program_address(
        &[b"global_config_account", authority.key().as_ref()],
        &crate::ID
    );

    if *global_config_account.key() != global_config_pda {
        return Err(ProgramError::InvalidAccountData);
    };

    let mut global_config_info = GlobalConfig::from_account_info_mut(global_config_account)?;
    
    if global_config_info.authority != *authority.key() {
        return Err(ProgramError::InvalidAccountData);
    };

    global_config_info.protocol_fee_rate = new_protocol_fee_rate;

    Ok(())
}
