use pinocchio::{account_info::{AccountInfo, Ref, RefMut}, program_error::ProgramError, pubkey::Pubkey, *};

pub trait AccountData: Sized {
    const SIZE: usize;
    
    fn from_account_info(account: &AccountInfo) -> Result<Ref<Self>, ProgramError> {
        if account.data_len() < Self::SIZE {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(Ref::map(account.try_borrow_data()?, |data| unsafe {
            &*(data.as_ptr() as *const Self)
        }))
    }

    fn from_account_info_mut(account: &AccountInfo) -> Result<RefMut<Self>, ProgramError> {
        if account.data_len() < Self::SIZE {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(RefMut::map(account.try_borrow_mut_data()?, |data| unsafe {
            &mut *(data.as_mut_ptr() as *mut Self)
        }))
    }
}
