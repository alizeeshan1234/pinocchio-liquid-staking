use pinocchio::{account_info::AccountInfo, pubkey::Pubkey, ProgramResult, program_error::ProgramError};
pub use pinocchio::*;
pub use pinocchio_pubkey::declare_id;

use crate::instructions::StakingInstructions;

declare_id!("4R8RFWJTUQH2iz8habt9SCu54pzQ6xB9CCZQ4bHAPveK");

entrypoint!(process_instruction);

pub mod instructions;
pub mod states;

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8]
) -> ProgramResult {

    let (ix_disc, instruction_data) = instruction_data.split_first().ok_or(ProgramError::InvalidInstructionData)?;

    match StakingInstructions::try_from(ix_disc)? {
        StakingInstructions::InitConfigAccount => instructions::init_global_config::process_initialize_global_config(accounts, instruction_data)?,
        StakingInstructions::UpdateAuthority => instructions::update_global_config::process_update_authority(accounts, instruction_data)?,
        StakingInstructions::UpdateProtocolFee => instructions::update_global_config::process_update_protocol_fee(accounts, instruction_data)?,
        StakingInstructions::CreateStakingPool => instructions::create_staking_pool::process_create_staking_pool(accounts, instruction_data)?,
        StakingInstructions::UpdatePoolConfig => instructions::update_pool_config::process_update_pool_config(accounts, instruction_data)?,
        StakingInstructions::InitOracleConfig => instructions::oracle::process_init_oracle_config(accounts, instruction_data)?,
        StakingInstructions::UpdateOraclePrice => instructions::oracle::process_update_price(accounts, instruction_data)?,
        StakingInstructions::GetOraclePrice => instructions::oracle::get_oracle_price(accounts)?,
        StakingInstructions::PausePool => instructions::pause_pool::process_pause_pool(accounts, instruction_data)?,
        StakingInstructions::ResumePool => instructions::resume_pool::process_resume_pool(accounts, instruction_data)?,
    };

    Ok(())
}