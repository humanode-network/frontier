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

use sp_runtime::traits::One;

pub use evm::{
	Config as EvmConfig, Context, ExitError, ExitFatal, ExitReason, ExitRevert, ExitSucceed,
};
pub use fp_evm::{
	Account, CallInfo, CreateInfo, ExecutionInfo, FeeCalculator, InvalidEvmTransactionError,
	LinearCostPrecompile, Log, Precompile, PrecompileFailure, PrecompileHandle, PrecompileOutput,
	PrecompileResult, PrecompileSet, Vicinity,
};

pub use pallet::*;

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

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new account was created.
		NewAccount { account: <T as Config>::AccountId },
		/// An account was reaped.
		KilledAccount { account: <T as Config>::AccountId },
	}

	#[pallet::storage]
	#[pallet::getter(fn account_nonces)]
	pub type AccountNonces<T: Config> =
		StorageMap<_, Blake2_128Concat, <T as Config>::AccountId, <T as Config>::Index, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn account_sufficients)]
	pub type AccountSufficients<T: Config> =
		StorageMap<_, Blake2_128Concat, <T as Config>::AccountId, u32, ValueQuery>;
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
		AccountNonces::<T>::get(who)
	}

	/// Increment a particular account's nonce by 1.
	pub fn inc_account_nonce(who: &<T as Config>::AccountId) {
		AccountNonces::<T>::mutate(who, |a| *a += <T as Config>::Index::one());
	}

	/// Increment the self-sufficient reference counter on an account.
	pub fn inc_sufficients(who: &<T as Config>::AccountId) {
		AccountSufficients::<T>::mutate(who, |sufficients| {
			if *sufficients == 0 {
				// Account is being created.
				*sufficients = 1;
				Self::on_created_account(who.clone());
			} else {
				*sufficients = sufficients.saturating_add(1);
			}
		})
	}

	/// Decrement the sufficients reference counter on an account.
	///
	/// This *MUST* only be done once for every time you called `inc_sufficients` on `who`.
	pub fn dec_sufficients(who: &<T as Config>::AccountId) {
		AccountSufficients::<T>::mutate_exists(who, |maybe_sufficients| {
			if let Some(mut sufficients) = maybe_sufficients.take() {
				if sufficients == 0 {
					// Logic error - cannot decrement beyond zero.
					log::error!(
						target: "frame-evm",
						"Logic error: Unexpected underflow in reducing sufficients",
					);
				}
				match sufficients {
					1 => {
						Pallet::<T>::on_killed_account(who.clone());
					},
					x => {
						sufficients = x - 1;
						*maybe_sufficients = Some(sufficients);
					},
				}
			} else {
				log::error!(
					target: "frame-evm",
					"Logic error: Account already dead when reducing provider",
				);
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
