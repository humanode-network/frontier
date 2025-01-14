// SPDX-License-Identifier: Apache-2.0

//! # EVM System Pallet.

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::traits::StoredMap;
use scale_codec::{Decode, Encode, FullCodec, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{traits::One, DispatchError, RuntimeDebug};

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use pallet::*;

/// Type used to encode the number of references an account has.
pub type RefCount = u32;

/// Account information.
#[derive(
	Clone,
	Eq,
	PartialEq,
	Default,
	RuntimeDebug,
	Encode,
	Decode,
	TypeInfo,
	MaxEncodedLen
)]
pub struct AccountInfo<Index, AccountData> {
	/// The number of transactions this account has sent.
	pub nonce: Index,
	/// The number of modules that allow this account to exist for their own purposes only. The
	/// account may not be reaped until this is zero.
	pub sufficients: RefCount,
	/// The additional data that belongs to this account. Used to store the balance(s) in a lot of
	/// chains.
	pub data: AccountData,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use sp_runtime::traits::{AtLeast32Bit, MaybeDisplay};
	use sp_std::fmt::Debug;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The user account identifier type.
		type AccountId: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ Debug
			+ MaybeDisplay
			+ Ord
			+ MaxEncodedLen;

		/// Account index (aka nonce) type. This stores the number of previous transactions
		/// associated with a sender account.
		type Index: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ Debug
			+ Default
			+ MaybeDisplay
			+ AtLeast32Bit
			+ Copy
			+ MaxEncodedLen;

		/// Data to be associated with an account (other than nonce/transaction counter, which this
		/// pallet does regardless).
		type AccountData: Member + FullCodec + Clone + Default + TypeInfo + MaxEncodedLen;

		/// Handler for when a new account has just been created.
		type OnNewAccount: OnNewAccount<<Self as Config>::AccountId>;

		/// A function that is invoked when an account has been determined to be dead.
		///
		/// All resources should be cleaned up associated with the given account.
		type OnKilledAccount: OnKilledAccount<<Self as Config>::AccountId>;
	}

	/// The full account information for a particular account ID.
	#[pallet::storage]
	pub type Account<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		<T as Config>::AccountId,
		AccountInfo<<T as Config>::Index, <T as Config>::AccountData>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new account was created.
		NewAccount { account: <T as Config>::AccountId },
		/// An account was reaped.
		KilledAccount { account: <T as Config>::AccountId },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The account already exists in case creating it.
		AccountAlreadyExist,
		/// The account doesn't exist in case removing it.
		AccountNotExist,
	}
}

/// The outcome of the account creation operation.
#[derive(Eq, PartialEq, RuntimeDebug)]
pub enum AccountCreationOutcome {
	/// Account was created.
	Created,
	/// Account already exists in the system, so action was taken.
	AlreadyExists,
}

/// The outcome of the account removal operation.
#[derive(Eq, PartialEq, RuntimeDebug)]
pub enum AccountRemovalOutcome {
	/// Account was destroyed and no longer exists.
	Reaped,
	/// Account was non-empty, and it was retained and still exists in the system.
	Retained,
	/// Account did not exist in the first place, so no action was taken.
	DidNotExist,
}

impl<T: Config> Pallet<T> {
	/// Check the account existence.
	pub fn account_exists(who: &<T as Config>::AccountId) -> bool {
		Account::<T>::contains_key(who)
	}

	/// The number of outstanding sufficient references for the account `who`.
	pub fn sufficients(who: &<T as Config>::AccountId) -> RefCount {
		Account::<T>::get(who).sufficients
	}

	/// An account is being created.
	fn on_created_account(who: <T as Config>::AccountId) {
		<T as Config>::OnNewAccount::on_new_account(&who);
		Self::deposit_event(Event::NewAccount { account: who });
	}

	/// Do anything that needs to be done after an account has been killed.
	fn on_killed_account(who: <T as Config>::AccountId) {
		<T as Config>::OnKilledAccount::on_killed_account(&who);
		Self::deposit_event(Event::KilledAccount { account: who });
	}

	/// Retrieve the account transaction counter from storage.
	pub fn account_nonce(who: &<T as Config>::AccountId) -> <T as Config>::Index {
		Account::<T>::get(who).nonce
	}

	/// Increment a particular account's nonce by 1.
	pub fn inc_account_nonce(who: &<T as Config>::AccountId) {
		Account::<T>::mutate(who, |a| a.nonce += <T as pallet::Config>::Index::one());
	}

	/// Create an account.
	pub fn create_account(who: &<T as Config>::AccountId) -> AccountCreationOutcome {
		Account::<T>::mutate(who, |a| {
			if a.sufficients == 0 {
				// Account is being created.
				a.sufficients = 1;
				Self::on_created_account(who.clone());
				AccountCreationOutcome::Created
			} else {
				a.sufficients = a.sufficients.saturating_add(1);
				AccountCreationOutcome::AlreadyExists
			}
		})
	}

	/// Remove an account.
	pub fn remove_account(who: &<T as Config>::AccountId) -> AccountRemovalOutcome {
		if !Self::account_exists(who) {
			return AccountRemovalOutcome::DidNotExist;
		}

		if Account::<T>::get(who).data != <T as Config>::AccountData::default() {
			return AccountRemovalOutcome::Retained;
		}

		Account::<T>::remove(who);
		Self::on_killed_account(who.clone());
		AccountRemovalOutcome::Reaped
	}
}

impl<T: Config> StoredMap<<T as Config>::AccountId, <T as Config>::AccountData> for Pallet<T> {
	fn get(k: &<T as Config>::AccountId) -> <T as Config>::AccountData {
		Account::<T>::get(k).data
	}

	fn try_mutate_exists<R, E: From<DispatchError>>(
		k: &<T as Config>::AccountId,
		f: impl FnOnce(&mut Option<<T as Config>::AccountData>) -> Result<R, E>,
	) -> Result<R, E> {
		let (mut maybe_account_data, was_providing) = if Self::account_exists(k) {
			(Some(Account::<T>::get(k).data), true)
		} else {
			(None, false)
		};

		let result = f(&mut maybe_account_data)?;

		match (maybe_account_data, was_providing) {
			(Some(data), false) => {
				Account::<T>::mutate(k, |a| a.data = data);
				Self::on_created_account(k.clone());
			}
			(Some(data), true) => {
				Account::<T>::mutate(k, |a| a.data = data);
			}
			(None, true) => {
				Account::<T>::remove(k);
				Self::on_killed_account(k.clone());
			}
			(None, false) => {
				// Do nothing.
			}
		}

		Ok(result)
	}
}

impl<T: Config> fp_evm::AccountProvider for Pallet<T> {
	type AccountId = <T as Config>::AccountId;
	type Index = <T as Config>::Index;

	fn create_account(who: &Self::AccountId) {
		let _ = Self::create_account(who);
	}

	fn remove_account(who: &Self::AccountId) {
		let _ = Self::remove_account(who);
	}

	fn account_nonce(who: &Self::AccountId) -> Self::Index {
		Self::account_nonce(who)
	}

	fn inc_account_nonce(who: &Self::AccountId) {
		Self::inc_account_nonce(who);
	}
}

/// Interface to handle account creation.
pub trait OnNewAccount<AccountId> {
	/// A new account `who` has been registered.
	fn on_new_account(who: &AccountId);
}

impl<AccountId> OnNewAccount<AccountId> for () {
	fn on_new_account(_who: &AccountId) {}
}

/// Interface to handle account killing.
pub trait OnKilledAccount<AccountId> {
	/// The account with the given id was reaped.
	fn on_killed_account(who: &AccountId);
}

impl<AccountId> OnKilledAccount<AccountId> for () {
	fn on_killed_account(_who: &AccountId) {}
}
