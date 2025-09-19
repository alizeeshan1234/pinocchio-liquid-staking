use std::default;

use pinocchio::{program_error::ProgramError, pubkey::Pubkey, *};
use crate::states::helper::AccountData;

pub const MAX_POSITIONS: usize = 10;
pub const MAX_HISTORY: usize = 10;

#[repr(C)]
#[derive(Clone, Debug)]
pub struct UserStakeAccount {
    pub owner: Pubkey,
    pub global_config: Pubkey,
    pub user_token_account: Pubkey,

    pub total_lst_balance: u64,
    pub total_staked_amount: u64,
    pub total_pending_rewards: u64,
    pub creation_timestamp: i64,

    pub active_positions: u8,
    pub is_paused: bool,
    pub positions: [StakePosition; MAX_POSITIONS], //A user can have multiple stake account so we keep the track of them inside the positions

    pub total_earned: u64,
    pub total_claimed: u64,
    pub pending_rewards: u64,
    pub last_claim_timestamp: i64,
    pub last_update_timestamp: i64,
    pub claim_history: [ClaimEvent; MAX_HISTORY],

    pub total_penalties: u64,
    pub active_penalties: u64,
    pub penalty_type_count: u8, // ( PenaltyType ) total penalty type count can be 4
    pub penalty_history: [PenaltyEvent; MAX_HISTORY],

    pub bump: u8,
}

impl AccountData for UserStakeAccount {
    const SIZE: usize = core::mem::size_of::<UserStakeAccount>();
}

#[repr(C)]
#[derive(Clone, Debug, Copy, Default)]
pub struct StakePosition {
    pub pool_id: u64,
    pub staking_pool: Pubkey,
    pub lst_token_account: Pubkey, //Need to create this account for the user when he stake his tokens
    pub staked_amount: u64,
    pub lst_tokens: u64,
    pub last_reward_update: i64,
    pub pending_rewards: u64,
    pub stake_timestamp: i64,
    pub lock_exipry_enable: bool,
    pub lock_expiry: i64,
    pub is_active: bool,
    pub bump: u8,
}

impl AccountData for StakePosition {
    const SIZE: usize = core::mem::size_of::<StakePosition>();
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ClaimEvent {
    pub amount: u64,
    pub timestamp: i64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PenaltyEvent {
    pub penalty_type: u8,
    pub penalty_id: u64,
    pub amount: u64,
    pub timestamp: i64,
    pub grace_period_end: i64,
    pub is_resolved: bool,
    pub resolution_timestamp: i64, // When penalty was resolved (0 if unresolved)
    pub pool_id: u64, // Which staking pool this penalty relates to
    pub user: Pubkey, // User who incurred the penalty
    pub validator: Pubkey, // Validator that caused the penalty 
    pub original_stake_amount: u64, // Original stake amount before penalty
    pub recovery_period: u32,  // Days until partial recovery 
}

pub enum PenaltyType {
    Slashing,
    EarlyUnstake,
    ValidatorMisbehavior,
    Inactivity
}

impl TryFrom<&u8> for PenaltyType {
    type Error = ProgramError;

    fn try_from(value: &u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(PenaltyType::Slashing),
            1 => Ok(PenaltyType::EarlyUnstake),
            2 => Ok(PenaltyType::ValidatorMisbehavior),
            3 => Ok(PenaltyType::Inactivity),
            _ => Err(ProgramError::InvalidAccountData)
        }
    }
}