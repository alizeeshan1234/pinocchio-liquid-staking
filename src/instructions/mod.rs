use pinocchio::program_error::ProgramError;
use shank::ShankInstruction;

pub mod init_global_config;

#[repr(u8)]
#[derive(ShankInstruction)]
pub enum StakingInstructions {
    #[account(0, writable, signer, name = "authority", desc = "Account that pays for account creation")]
    #[account(1, name = "mint", desc = "Mint Account")]
    #[account(2, writable, name = "global_config_account", desc = "GLobal Config Account")]
    #[account(3, writable, name = "treasury_account", desc = "treasury")]
    #[account(4, name = "system_program", desc = "System program")]
    #[account(5, name = "token_program", desc = "Token Program")]
    InitConfigAccount = 0
}

impl TryFrom<&u8> for StakingInstructions {
    type Error = ProgramError;

    fn try_from(value: &u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(StakingInstructions::InitConfigAccount),
            _ => Err(ProgramError::InvalidInstructionData)
        }
    }
}