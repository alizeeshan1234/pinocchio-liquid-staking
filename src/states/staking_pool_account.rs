use pinocchio::{account_info::{AccountInfo, Ref, RefMut}, program_error::ProgramError, pubkey::Pubkey, *};
use shank::ShankAccount;
use crate::states::helper::AccountData;

#[derive(Debug, Clone, ShankAccount)]
pub struct StakingPool {
    pub authority: Pubkey,
    pub pool_id: u64,
    pub creation_timestamp: i64,
    pub pool_status: u8, //PoolStateEnum
    pub stake_token_mint: Pubkey,
    pub reward_token_mint: Pubkey,
    pub stake_token_vault: Pubkey,
    pub reward_token_vault: Pubkey,
    pub total_staked: u64,
    pub total_reward_distributed: u64,
    pub reward_rate_per_second: u64, //u
    pub accumulated_reward_per_share: u128, //u
    pub lock_period_enabled: bool, //u
    pub lock_period_duration: i64, //u
    pub reward_multiplier: u16, //u
    pub early_withdraw_penalty: u64, //u
    pub slashing_enabled: bool, //u
    pub slashing_condition_type: u8, //SlashTypeEnum //u
    pub slash_percentage: u16, //u
    pub min_evidence_required: u8, //u
    pub cooldown_period: i64, //u
    pub price_feed_account: Pubkey, //u
    pub maximum_stake_limit: u64, //u
    pub minimum_stake_amount: u64, //u
    pub liquid_stake_mint: Pubkey, 
    pub liquid_stake_supply: u64,
    pub emergency_pause_flag: bool, 
    pub stake_pool_bump: u8,
}

pub enum PoolStatusEnum {
    Active,
    Paused,
    Deprecated,
    Emergency
}

impl TryFrom<&u8> for PoolStatusEnum {
    type Error = ProgramError;

    fn try_from(value: &u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(PoolStatusEnum::Active),
            1 => Ok(PoolStatusEnum::Paused),
            2 => Ok(PoolStatusEnum::Deprecated),
            3 => Ok(PoolStatusEnum::Emergency),
            _ => Err(ProgramError::InvalidAccountData)
        }
    }
}

pub enum SlashTypeEnum {
    DownTime,
    DoubleSign,
    InvalidAttestation,
    Censorship,
    Custom
}

impl TryFrom<&u8> for SlashTypeEnum {
    type Error = ProgramError;

    fn try_from(value: &u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(SlashTypeEnum::DownTime),
            1 => Ok(SlashTypeEnum::DoubleSign),
            2 => Ok(SlashTypeEnum::InvalidAttestation),
            3 => Ok(SlashTypeEnum::Censorship),
            4 => Ok(SlashTypeEnum::Custom),
            _ => Err(ProgramError::InvalidAccountData)
        }
    }
}

impl AccountData for StakingPool {
    const SIZE: usize = core::mem::size_of::<StakingPool>();
}