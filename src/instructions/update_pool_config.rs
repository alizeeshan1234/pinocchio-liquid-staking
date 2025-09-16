use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey, *};
use crate::states::{helper::AccountData, staking_pool_account::{PoolStatusEnum, SlashTypeEnum, StakingPool}};

#[derive(Debug)]
pub enum PoolUpdateType {
    RewardRatePerSecond(u64),
    LockPeriodDuration(i64),
    RewardMultiplier(u16),
    EarlyWithdrawPenalty(u64),
    SlashPercentage(u16),
    MinEvidenceRequired(u8),
    CooldownPeriod(i64),
    MaximumStakeLimit(u64),
    MinimumStakeAmount(u64),
    LockPeriodEnabled(bool),
    SlashingEnabled(bool),
    SlashingConditionType(u8),
    PriceFeedAccount(Pubkey),
    PoolStatus(u8),
    EmergencyPause(bool),
}

pub fn process_update_pool_config(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {

    let [authority, staking_pool_account, price_feed_account] = accounts else {
        return Err(ProgramError::InvalidAccountData);
    };

    if !authority.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    };

    let update_type_discriminator = instruction_data[0];
    let pool_id = u64::from_le_bytes(
        instruction_data[1..9].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );

    let update_type = match update_type_discriminator {
        0 => {
            let value = u64::from_le_bytes(
                instruction_data[9..17].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
            );
            if value == 0 {
                return Err(ProgramError::InvalidInstructionData); 
            }
            PoolUpdateType::RewardRatePerSecond(value)
        },
        1 => {
            let value = i64::from_le_bytes(
                instruction_data[9..17].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
            );
            if value <= 0 {
                return Err(ProgramError::InvalidInstructionData);
            };
            PoolUpdateType::LockPeriodDuration(value)
        },
        2 => {
            let value = u16::from_le_bytes(
                instruction_data[9..11].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
            );
            if value == 0 {
                return Err(ProgramError::InvalidInstructionData); 
            };
            PoolUpdateType::RewardMultiplier(value)
        },
        3 => {
            let value = u64::from_le_bytes(
                instruction_data[9..17].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
            );
            // Early withdraw penalty can be 0 (no penalty)
            PoolUpdateType::EarlyWithdrawPenalty(value)
        },
        4 => {
            let value = u16::from_le_bytes(
                instruction_data[9..11].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
            );
            // Validate percentage is not more than 100% (10000 basis points)
            if value > 10000 {
                return Err(ProgramError::InvalidInstructionData); 
            };
            PoolUpdateType::SlashPercentage(value)
        },
        5 => {
            let value = instruction_data[9];
            if value == 0 {
                return Err(ProgramError::InvalidInstructionData); 
            };
            PoolUpdateType::MinEvidenceRequired(value)
        },
        6 => {
            let value = i64::from_le_bytes(
                instruction_data[9..17].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
            );
            if value <= 0 {
                return Err(ProgramError::InvalidInstructionData);
            };
            PoolUpdateType::CooldownPeriod(value)
        },
        7 => {
            let value = u64::from_le_bytes(
                instruction_data[9..17].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
            );
            // Allow 0 for unlimited staking (consistent with create pool)
            PoolUpdateType::MaximumStakeLimit(value)
        },
        8 => {
            let value = u64::from_le_bytes(
                instruction_data[9..17].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
            );
            if value == 0 {
                return Err(ProgramError::InvalidInstructionData);
            };
            PoolUpdateType::MinimumStakeAmount(value)
        },
        9 => {
            let value = instruction_data[9];
            let value_type = match value {
                0 => false,  // 0 = disabled
                1 => true,   // 1 = enabled
                _ => return Err(ProgramError::InvalidInstructionData),
            };
            PoolUpdateType::LockPeriodEnabled(value_type)
        },
        10 => {
            let value = instruction_data[9];
            let value_type = match value {
                0 => false,  // 0 = disabled
                1 => true,   // 1 = enabled
                _ => return Err(ProgramError::InvalidInstructionData),
            };
            PoolUpdateType::SlashingEnabled(value_type)
        },
        11 => {
            let value = instruction_data[9];
            SlashTypeEnum::try_from(&value)?;
            PoolUpdateType::SlashingConditionType(value)
        },
        12 => {
            let value = *price_feed_account.key();
            PoolUpdateType::PriceFeedAccount(value)
        },
        13 => {
            let value = instruction_data[9];
            PoolStatusEnum::try_from(&value)?;
            PoolUpdateType::PoolStatus(value)
        },
        14 => {
            let value = instruction_data[9];
            let value_type = match value {
                0 => false,  // 0 = not paused
                1 => true,   // 1 = paused
                _ => return Err(ProgramError::InvalidInstructionData),
            };
            PoolUpdateType::EmergencyPause(value_type)
        },
        _ => {
            return Err(ProgramError::InvalidInstructionData);
        }
    };

    let mut staking_pool_account_info = StakingPool::from_account_info_mut(staking_pool_account)?;

    if staking_pool_account_info.authority != *authority.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    if staking_pool_account_info.pool_id != pool_id {
        return Err(ProgramError::InvalidAccountData);
    }

    if staking_pool_account_info.emergency_pause_flag {
        match update_type {
            PoolUpdateType::EmergencyPause(_) | 
            PoolUpdateType::PoolStatus(_) => {},
            _ => {
                return Err(ProgramError::InvalidAccountData);
            }
        }
    }

    match update_type {
        PoolUpdateType::RewardRatePerSecond(value) => {
            staking_pool_account_info.reward_rate_per_second = value;
        },
        PoolUpdateType::LockPeriodDuration(value) => {
            staking_pool_account_info.lock_period_duration = value;
        },
        PoolUpdateType::RewardMultiplier(value) => {
            staking_pool_account_info.reward_multiplier = value;
        },
        PoolUpdateType::EarlyWithdrawPenalty(value) => {
            staking_pool_account_info.early_withdraw_penalty = value;
        },
        PoolUpdateType::SlashPercentage(value) => {
            staking_pool_account_info.slash_percentage = value;
        },
        PoolUpdateType::MinEvidenceRequired(value) => {
            staking_pool_account_info.min_evidence_required = value;
        },
        PoolUpdateType::CooldownPeriod(value) => {
            staking_pool_account_info.cooldown_period = value;
        },
        PoolUpdateType::MaximumStakeLimit(value) => {
            if value != 0 {
                if value < staking_pool_account_info.total_staked {
                    return Err(ProgramError::InvalidInstructionData);
                }
                if value < staking_pool_account_info.minimum_stake_amount {
                    return Err(ProgramError::InvalidInstructionData);
                }
            }
            staking_pool_account_info.maximum_stake_limit = value;
        },
        PoolUpdateType::MinimumStakeAmount(value) => {
            if staking_pool_account_info.maximum_stake_limit != 0 && 
               value > staking_pool_account_info.maximum_stake_limit {
                return Err(ProgramError::InvalidInstructionData);
            }
            staking_pool_account_info.minimum_stake_amount = value;
        },
        PoolUpdateType::LockPeriodEnabled(value) => {
            staking_pool_account_info.lock_period_enabled = value;
        },
        PoolUpdateType::SlashingEnabled(value) => {
            staking_pool_account_info.slashing_enabled = value;
        },
        PoolUpdateType::SlashingConditionType(value) => {
            staking_pool_account_info.slashing_condition_type = value;
        },
        PoolUpdateType::PriceFeedAccount(value) => {
            staking_pool_account_info.price_feed_account = value;
        },
        PoolUpdateType::PoolStatus(value) => {
            staking_pool_account_info.pool_status = value;
        },
        PoolUpdateType::EmergencyPause(value) => {
            staking_pool_account_info.emergency_pause_flag = value;
            if value {
                staking_pool_account_info.pool_status = 3; 
            }
        },
    }

    Ok(())
}