use pinocchio::{pubkey::Pubkey, *};
use crate::states::helper::AccountData;

pub const MAX_POSITIONS: usize = 10;

#[repr(C)]
#[derive(Clone, Debug)]
pub struct UserStakeAccount {
    pub owner: Pubkey,
    pub global_config: Pubkey,
    pub user_token_account: Pubkey,
    pub total_lst_balance: u64,
    pub total_staked_amount: u64,
    pub total_pending_rewards: u64,
    pub active_positions: u8,
    pub creation_timestamp: i64,
    pub is_paused: bool,
    pub positions: [StakePosition; MAX_POSITIONS], //A user can have multiple stake account so we keep the track of them inside the positions
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