use pinocchio::{account_info::{AccountInfo, Ref, RefMut}, program_error::ProgramError, pubkey::Pubkey, *};
use shank::ShankAccount;
use crate::states::helper::AccountData;

#[derive(Debug, Clone, ShankAccount)]
pub struct GlobalConfig {
    pub authority: Pubkey,
    pub treasury: Pubkey,
    pub protocol_fee_rate: u16,  
    pub max_pools: u32,      
    pub min_stake_amount: u64,
    pub emergency_pause: bool, 
    pub total_pools_created: u64,
    pub active_pools: u64,
    pub active_pool_keys: Vec<Pubkey>,
    pub bump: u8, 
    pub treasury_bump: u8,
}

impl AccountData for GlobalConfig {
    const SIZE: usize = core::mem::size_of::<GlobalConfig>();
}
