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

//! # EVM System Pallet

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

use sp_runtime::{traits::One, RuntimeDebug};
use scale_codec::{Encode, Decode, MaxEncodedLen};
use scale_info::TypeInfo;

pub use pallet::*;

/// Type used to encode the number of references an account has.
pub type RefCount = u32;

/// Some resultant status relevant to incrementing a provider/self-sufficient reference.
#[derive(Eq, PartialEq, RuntimeDebug)]
pub enum IncRefStatus {
	/// Account was created.
	Created,
	/// Account already existed.
	Existed,
}

/// Some resultant status relevant to decrementing a provider/self-sufficient reference.
#[derive(Eq, PartialEq, RuntimeDebug)]
pub enum DecRefStatus {
	/// Account was destroyed.
	Reaped,
	/// Account still exists.
	Exists,
}

/// Information of an account.
#[derive(Clone, Eq, PartialEq, Default, RuntimeDebug, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct AccountInfo<Index> {
	/// The number of transactions this account has sent.
	pub nonce: Index,
	/// The number of other modules that currently depend on this account's existence. The account
	/// cannot be reaped until this is zero.
	pub consumers: RefCount,
	/// The number of other modules that allow this account to exist. The account may not be reaped
	/// until this and `sufficients` are both zero.
	pub providers: RefCount,
	/// The number of modules that allow this account to exist for their own purposes only. The
	/// account may not be reaped until this and `providers` are both zero.
	pub sufficients: RefCount,
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
		AccountInfo<<T as Config>::Index>,
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
}

impl<T: Config> Pallet<T> {
	/// An account is being created.
	pub fn on_created_account(who: <T as Config>::AccountId) {
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
		FullAccount::<T>::get(who).nonce
	}

	/// Increment a particular account's nonce by 1.
	pub fn inc_account_nonce(who: &<T as Config>::AccountId) {
		FullAccount::<T>::mutate(who, |a| a.nonce += <T as pallet::Config>::Index::one());
	}

	/// Increment the self-sufficient reference counter on an account.
	pub fn inc_sufficients(who: &<T as Config>::AccountId) -> IncRefStatus {
		FullAccount::<T>::mutate(who, |a| {
			if a.providers + a.sufficients == 0 {
				// Account is being created.
				a.sufficients = 1;
				Self::on_created_account(who.clone());
				IncRefStatus::Created
			} else {
				a.sufficients = a.sufficients.saturating_add(1);
				IncRefStatus::Existed
			}
		})
	}

	/// Decrement the sufficients reference counter on an account.
	///
	/// This *MUST* only be done once for every time you called `inc_sufficients` on `who`.
	pub fn dec_sufficients(who: &<T as Config>::AccountId) -> DecRefStatus {
		FullAccount::<T>::mutate_exists(who, |maybe_account| {
			if let Some(mut account) = maybe_account.take() {
				if account.sufficients == 0 {
					// Logic error - cannot decrement beyond zero.
					log::error!(
						target: "frame-evm-system",
						"Logic error: Unexpected underflow in reducing sufficients",
					);
				}
				match (account.sufficients, account.providers) {
					(0, 0) | (1, 0) => {
						Pallet::<T>::on_killed_account(who.clone());
						DecRefStatus::Reaped
					},
					(x, _) => {
						account.sufficients = x - 1;
						*maybe_account = Some(account);
						DecRefStatus::Exists
					},
				}
			} else {
				log::error!(
					target: "frame-evm-system",
					"Logic error: Account already dead when reducing provider",
				);
				DecRefStatus::Reaped
			}
		})
	}
}

pub trait OnNewAccount<AccountId> {
	/// A new account `who` has been registered.
	fn on_new_account(who: &AccountId);
}

pub trait OnKilledAccount<AccountId> {
	/// The account with the given id was reaped.
	fn on_killed_account(who: &AccountId);
}
