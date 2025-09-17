use pinocchio::{account_info::{AccountInfo, Ref, RefMut}, program_error::ProgramError, pubkey::Pubkey, *};
use shank::ShankAccount;
use crate::states::helper::AccountData;

#[derive(Debug, Clone, ShankAccount)]
pub struct OracleConfigInfo {
    pub price_feed_account: Pubkey,
    pub update_frequency_seconds: i64,
    pub oracle_authority: Pubkey,
    pub last_update_timestamp: i64,
    pub current_price: u64,
    pub oracle_account_bump: u8,
}

impl AccountData for OracleConfigInfo {
    const SIZE: usize = core::mem::size_of::<OracleConfigInfo>();
}