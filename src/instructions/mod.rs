use pinocchio::program_error::ProgramError;
use shank::ShankInstruction;

pub mod init_global_config;
pub mod update_global_config;
pub mod create_staking_pool;
pub mod update_pool_config;
pub mod oracle;

#[repr(u8)]
#[derive(ShankInstruction)]
pub enum StakingInstructions {
    #[account(0, writable, signer, name = "authority", desc = "Account that pays for account creation")]
    #[account(1, name = "mint", desc = "Mint Account")]
    #[account(2, writable, name = "global_config_account", desc = "GLobal Config Account")]
    #[account(3, writable, name = "treasury_account", desc = "treasury")]
    #[account(4, name = "system_program", desc = "System program")]
    #[account(5, name = "token_program", desc = "Token Program")]
    InitConfigAccount = 0,

    #[account(0, writable, signer, name = "authority", desc = "Account that pays for account creation")]
    #[account(1, writable, name = "global_config_account", desc = "GLobal Config Account")]
    UpdateAuthority = 1,

    #[account(0, writable, signer, name = "authority", desc = "Account that pays for account creation")]
    #[account(1, writable, name = "global_config_account", desc = "GLobal Config Account")]
    UpdateProtocolFee = 2,

    #[account(0, writable, name = "authority", desc = "Account that pays for account creation")]
    #[account(1, writable, signer, name = "creator", desc = "Account that pays for account creation")]
    #[account(2, name = "stake_token_mint", desc = "Account that pays for account creation")]
    #[account(3, name = "reward_token_mint", desc = "Account that pays for account creation")]
    #[account(4, name = "stake_token_vault", desc = "Account that pays for account creation")]
    #[account(5, name = "reward_token_vault", desc = "Account that pays for account creation")]
    #[account(6, writable, name = "staking_pool_account", desc = "Account that pays for account creation")]
    #[account(7, writable, name = "global_config_account", desc = "Account that pays for account creation")]
    #[account(8, writable, name = "liquid_stake_mint", desc = "Liquid stake mint")]
    #[account(9, name = "price_feed_account", desc = "Price feed account")]
    #[account(10, name = "system_program", desc = "System program")]
    #[account(11, name = "token_program", desc = "Token program")]
    CreateStakingPool = 3,

    #[account(0, writable, name = "authority", desc = "Account that pays for account creation")]
    #[account(1, writable, name = "staking_pool_account", desc = "Account that pays for account creation")]
    #[account(2, name = "price_feed_account", desc = "Account that pays for account creation")]
    UpdatePoolConfig = 4,

    #[account(0, writable, signer, name = "oracle_authority", desc = "Authority that can update oracle config")]
    #[account(1, writable, name = "oracle_config_account", desc = "Oracle configuration account to be created")]
    #[account(2, name = "price_feed_account", desc = "Price feed account for oracle data")]
    #[account(3, name = "system_program", desc = "System program for account creation")]
    InitOracleConfig = 5,

    #[account(0, writable, signer, name = "oracle_authority", desc = "Authority that can update oracle config")]
    #[account(1, writable, name = "oracle_config_account", desc = "Oracle configuration account to be created")]
    UpdateOraclePrice = 6,

    #[account(0, name = "oracle_authority", desc = "Authority used for PDA derivation")]
    #[account(1, name = "oracle_config_account", desc = "Oracle configuration account to read from")]
    GetOraclePrice = 7,
}

impl TryFrom<&u8> for StakingInstructions {
    type Error = ProgramError;

    fn try_from(value: &u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(StakingInstructions::InitConfigAccount),
            1 => Ok(StakingInstructions::UpdateAuthority),
            2 => Ok(StakingInstructions::UpdateProtocolFee),
            3 => Ok(StakingInstructions::CreateStakingPool),
            4 => Ok(StakingInstructions::UpdatePoolConfig),
            5 => Ok(StakingInstructions::InitOracleConfig),
            6 => Ok(StakingInstructions::UpdateOraclePrice),
            7 => Ok(StakingInstructions::GetOraclePrice),
            _ => Err(ProgramError::InvalidInstructionData)
        }
    }
}