//! Custom account provider logic.

use super::*;

/// The account provider interface abstraction layer.
///
/// Expose account related logic that `pallet_evm` required to control accounts existence
/// in the network and their transactions uniqueness. By default, the pallet operates native
/// system accounts records that `frame_system` provides.
///
/// The interface allow any custom account provider logic to be used instead of
/// just using `frame_system` account provider. The accounts records should store nonce value
/// for each account at least.
pub trait AccountProvider {
	/// The account identifier type.
	///
	/// Represent the account itself in accounts records.
	type AccountId;
	/// Account index (aka nonce) type.
	///
	/// The number that helps to ensure that each transaction in the network is unique
	/// for particular account.
	type Index: AtLeast32Bit;

	/// Creates a new account in accounts records.
	fn create_account(who: &Self::AccountId);
	/// Removes an account from accounts records.
	fn remove_account(who: &Self::AccountId);
	/// Return current account nonce value.
	fn account_nonce(who: &Self::AccountId) -> Self::Index;
	/// Increment a particular account's nonce value.
	fn inc_account_nonce(who: &Self::AccountId);
}

/// Native system account provider that `frame_system` provides.
pub struct NativeSystemAccountProvider<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> AccountProvider for NativeSystemAccountProvider<T> {
	type AccountId = <T as frame_system::Config>::AccountId;
	type Index = <T as frame_system::Config>::Index;

	fn account_nonce(who: &Self::AccountId) -> Self::Index {
		frame_system::Pallet::<T>::account_nonce(&who)
	}

	fn inc_account_nonce(who: &Self::AccountId) {
		frame_system::Pallet::<T>::inc_account_nonce(&who)
	}

	fn create_account(who: &Self::AccountId) {
		let _ = frame_system::Pallet::<T>::inc_sufficients(&who);
	}
	fn remove_account(who: &Self::AccountId) {
		let _ = frame_system::Pallet::<T>::dec_sufficients(&who);
	}
}
