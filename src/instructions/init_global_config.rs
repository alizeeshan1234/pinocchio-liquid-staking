use pinocchio::{account_info::AccountInfo, instruction::Signer, program_error::ProgramError, sysvars::{rent::Rent, Sysvar}, *};
use pinocchio_system::instructions::CreateAccount;
use pinocchio_token::{instructions::InitializeAccount3, state::TokenAccount};

use crate::states::{global_config::GlobalConfig, helper::AccountData};

pub fn process_initialize_global_config(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    
    let [authority, mint, global_config_account, treasury_account, system_program, token_program] = accounts else {
        return Err(ProgramError::InvalidAccountData);
    };

    if !authority.is_signer() {
        return Err(ProgramError::InvalidAccountData);
    };

    if instruction_data.len() < 14 {
        return Err(ProgramError::InvalidInstructionData);
    };

    let protocol_fee_rate = u16::from_le_bytes(
        instruction_data[0..2].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );

    let min_stake_amount = u64::from_le_bytes(
        instruction_data[2..10].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );

    let max_pools = u32::from_le_bytes(
        instruction_data[10..14].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );

    if protocol_fee_rate > 10000 { 
        return Err(ProgramError::InvalidArgument);
    };

    if min_stake_amount == 0 {
        return Err(ProgramError::InvalidArgument);
    };

    if max_pools == 0 {
        return Err(ProgramError::InvalidArgument);
    };

    let (global_config_pda, bump1) = pubkey::find_program_address(
        &[b"global_config_account", authority.key().as_ref()],
        &crate::ID
    );

    let (treasury_account_pda, bump2) = pubkey::find_program_address(
        &[b"treasury_account", mint.key().as_ref(), authority.key().as_ref()],
        &crate::ID
    );

    if *global_config_account.key() != global_config_pda {
        return Err(ProgramError::InvalidAccountData);
    };

    if *treasury_account.key() != treasury_account_pda {
        return Err(ProgramError::InvalidAccountData);
    };

    if treasury_account.data_is_empty() {
        let lamports = Rent::get()?.minimum_balance(TokenAccount::LEN);

        let bump_ref = &[bump2];
        let seeds = seeds!(
            b"treasury_account", 
            mint.key().as_ref(), 
            authority.key().as_ref(),
            bump_ref
        );
        let signer_seeds = Signer::from(&seeds);

        CreateAccount {
            from: authority,
            to: treasury_account,
            lamports,
            space: TokenAccount::LEN as u64,
            owner: &pinocchio_token::ID
        }.invoke_signed(&[signer_seeds])?;

        InitializeAccount3 {
            account: treasury_account,
            mint,
            owner: &authority.key()
        }.invoke()?;
    } else {
        return Err(ProgramError::AccountAlreadyInitialized);
    };

    if global_config_account.data_is_empty() {
        let lamports = Rent::get()?.minimum_balance(GlobalConfig::SIZE);

        let bump_ref = &[bump1];
        let seeds = seeds!(
            b"global_config_account", 
            authority.key().as_ref(),
            bump_ref
        );
        let signer_seeds = Signer::from(&seeds);

        CreateAccount {
            from: authority,
            to: global_config_account,
            lamports,
            space: GlobalConfig::SIZE as u64,
            owner: &crate::ID
        }.invoke_signed(&[signer_seeds])?;

        let mut global_config_account_info = GlobalConfig::from_account_info_mut(global_config_account)?;
        global_config_account_info.authority = *authority.key();
        global_config_account_info.treasury = *treasury_account.key();
        global_config_account_info.protocol_fee_rate = protocol_fee_rate;
        global_config_account_info.max_pools = max_pools;
        global_config_account_info.min_stake_amount = min_stake_amount;
        global_config_account_info.emergency_pause = true;
        global_config_account_info.total_pools_created = 0;
        global_config_account_info.active_pools = 0;
        global_config_account_info.active_pool_keys = Vec::new();
        global_config_account_info.bump = bump1;
        global_config_account_info.treasury_bump = bump2;
    } else {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    Ok(())
}

// ====================== TESTING process_initialize_global_config ======================
// #[cfg(test)]
// mod testing {
//     use std::vec;

//     use super::*;
//     use mollusk_svm::{program, Mollusk, result::Check};
//     use pinocchio_token::state::Mint;
//     use solana_sdk::{
//         account::Account,
//         instruction::{AccountMeta, Instruction},
//         pubkey::Pubkey,
//         pubkey,
//         system_program
//     };

//     const PROGRAM_ID: Pubkey = pubkey!("HP1VTBW6YdDjLDYNpn94EpaHz6QJ2LHKjBuGeuJJGjES");
//     const AUTHORITY: Pubkey = Pubkey::new_from_array([9u8; 32]);
//     const MINT: Pubkey = Pubkey::new_from_array([7u8; 32]);

//     #[test]
//     fn test_process_initialize_global_config() {
//         // Correct path to the BPF program
//         let mollusk = Mollusk::new(&PROGRAM_ID, "target/deploy/staking_platform");

//         let (global_config_pda, _bump1) = Pubkey::find_program_address(
//             &[b"global_config_account", AUTHORITY.as_ref()],
//             &PROGRAM_ID
//         );  

//         let (treasury_account_pda, _bump2) = Pubkey::find_program_address(
//             &[b"treasury_account", MINT.as_ref(), AUTHORITY.as_ref()],
//             &PROGRAM_ID
//         );   

//         // Use mollusk's built-in program support for system program
//         let (system_program_id, system_account) = program::keyed_account_for_system_program();

//         let token_program_account = Account {
//             lamports: 0,
//             data: vec![],
//             owner: Pubkey::default(),
//             executable: true,
//             rent_epoch: 0,
//         };

//         let protocol_fee_rate: u16 = 500; // 5%
//         let min_stake_amount: u64 = 1000000; // 1 SOL in lamports
//         let max_pools: u32 = 100;

//         // Create instruction data with discriminator byte first
//         let mut instruction_data = vec![0u8; 15]; // 1 byte discriminator + 14 bytes data
//         instruction_data[0] = 0; // Discriminator for InitConfigAccount instruction
//         instruction_data[1..3].copy_from_slice(&protocol_fee_rate.to_le_bytes());
//         instruction_data[3..11].copy_from_slice(&min_stake_amount.to_le_bytes());
//         instruction_data[11..15].copy_from_slice(&max_pools.to_le_bytes());

//         let instruction = Instruction {
//             program_id: PROGRAM_ID,
//             accounts: vec![
//                 AccountMeta::new(AUTHORITY, true),
//                 AccountMeta::new_readonly(MINT, false),
//                 AccountMeta::new(global_config_pda, false),
//                 AccountMeta::new(treasury_account_pda, false),
//                 AccountMeta::new_readonly(system_program_id, false),
//                 AccountMeta::new_readonly(pinocchio_token::id().into(), false),
//             ],
//             data: instruction_data
//         };

//         let authority_account = Account {
//             lamports: 10_000_000,
//             data: vec![],
//             owner: solana_sdk::system_program::id(),
//             executable: false,
//             rent_epoch: 0,
//         };

//         let mut mint_data = vec![0u8; Mint::LEN];
//         mint_data[0] = 1; 
//         mint_data[36] = 1; 
//         mint_data[44] = 9; 

//         let mint_account = Account {
//             lamports: 1000000,
//             data: mint_data,
//             owner: pinocchio_token::id().into(),
//             executable: false,
//             rent_epoch: 0,
//         };

//         let global_config_account = Account {
//             lamports: 0,
//             data: vec![],
//             owner: system_program::id(),
//             executable: false,
//             rent_epoch: 0,
//         };

//         let treasury_account = Account {
//             lamports: 0,
//             data: vec![],
//             owner: system_program::id(), 
//             executable: false,
//             rent_epoch: 0,
//         };

//         mollusk.process_and_validate_instruction(
//             &instruction,
//             &vec![
//                 (AUTHORITY, authority_account),
//                 (MINT, mint_account),
//                 (global_config_pda, global_config_account),
//                 (treasury_account_pda, treasury_account),
//                 (system_program_id, system_account),
//                 (pinocchio_token::id().into(), token_program_account),
//             ],
//             &[Check::success()],
//         );
//     }
// }