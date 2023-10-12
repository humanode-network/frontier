// SPDX-License-Identifier: Apache-2.0
// This file is part of Frontier.
//
// Copyright (c) 2020-2022 Parity Technologies (UK) Ltd.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # EVM System Pallet.

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

use sp_runtime::{traits::One, RuntimeDebug, DispatchResult};
use scale_codec::{Encode, Decode, MaxEncodedLen, FullCodec};
use scale_info::TypeInfo;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use pallet::*;

/// Account information.
#[derive(Clone, Eq, PartialEq, Default, RuntimeDebug, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct AccountInfo<Nonce, AccountData> {
	/// The number of transactions this account has sent.
	pub nonce: Nonce,
	/// The additional data that belongs to this account. Used to store the balance(s) in a lot of
	/// chains.
	pub data: AccountData,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use sp_runtime::traits::{MaybeDisplay, AtLeast32Bit};
	use sp_std::fmt::Debug;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
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

		/// Nonce type. This stores the number of previous transactions
		/// associated with a sender account.
		type Nonce: Parameter
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
	#[pallet::getter(fn full_account)]
	pub type FullAccount<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		<T as Config>::AccountId,
		AccountInfo<<T as Config>::Nonce, <T as Config>::AccountData>,
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

impl<T: Config> Pallet<T> {
	/// Check the account existence.
	pub fn account_exists(who: &<T as Config>::AccountId) -> bool {
		FullAccount::<T>::contains_key(who)
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
	pub fn account_nonce(who: &<T as Config>::AccountId) -> <T as Config>::Nonce {
		FullAccount::<T>::get(who).nonce
	}

	/// Increment a particular account's nonce by 1.
	pub fn inc_account_nonce(who: &<T as Config>::AccountId) {
		FullAccount::<T>::mutate(who, |a| a.nonce += <T as pallet::Config>::Nonce::one());
	}

	/// Create an account.
	pub fn create_account(who: &<T as Config>::AccountId) -> DispatchResult {
		if Self::account_exists(who) {
			return Err(Error::<T>::AccountAlreadyExist.into());
		}

		FullAccount::<T>::insert(who.clone(), AccountInfo::<_, _>::default());
		Self::on_created_account(who.clone());
		Ok(())
	}

	/// Remove an account.
	pub fn remove_account(who: &<T as Config>::AccountId) -> DispatchResult {
		if !Self::account_exists(who) {
			return Err(Error::<T>::AccountNotExist.into());
		}

		FullAccount::<T>::remove(who);
		Self::on_killed_account(who.clone());
		Ok(())
	}
}

/// Interface to handle account creation.
pub trait OnNewAccount<AccountId> {
	/// A new account `who` has been registered.
	fn on_new_account(who: &AccountId);
}

/// Interface to handle account killing.
pub trait OnKilledAccount<AccountId> {
	/// The account with the given id was reaped.
	fn on_killed_account(who: &AccountId);
}
