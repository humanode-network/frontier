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

//! # EVM Balances Pallet.

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::traits::{StorageVersion, OnUnbalanced, StoredMap, Imbalance};
use sp_runtime::{traits::{One, Zero}, RuntimeDebug, DispatchResult, Saturating};
use scale_codec::{Codec, Encode, Decode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_std::{cmp, fmt::Debug, result};

pub mod account_data;
use account_data::AccountData;

mod imbalances;
pub use imbalances::{NegativeImbalance, PositiveImbalance};

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use pallet::*;

/// The current storage version.
const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use sp_runtime::{
        traits::{AtLeast32BitUnsigned, MaybeDisplay},
        FixedPointOperand,
    };
	use sp_std::fmt::Debug;

	#[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self, I>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The user account identifier type.
		type AccountId: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ Debug
			+ MaybeDisplay
			+ Ord
			+ MaxEncodedLen;

		/// The balance of an account.
        type Balance: Parameter
            + Member
            + AtLeast32BitUnsigned
            + Codec
            + Default
            + Copy
            + MaybeSerializeDeserialize
            + Debug
            + MaxEncodedLen
            + TypeInfo
            + FixedPointOperand;

		/// The minimum amount required to keep an account open.
        #[pallet::constant]
        type ExistentialDeposit: Get<Self::Balance>;

		/// The means of storing the balances of an account.
		type AccountStore: StoredMap<<Self as Config<I>>::AccountId, AccountData<Self::Balance>>;

        /// Handler for the unbalanced reduction when removing a dust account.
        type DustRemoval: OnUnbalanced<NegativeImbalance<Self, I>>;
	}

	/// The total units issued.
    #[pallet::storage]
    #[pallet::getter(fn total_issuance)]
    #[pallet::whitelist_storage]
    pub type TotalIssuance<T: Config<I>, I: 'static = ()> = StorageValue<_, T::Balance, ValueQuery>;

	/// The total units of outstanding deactivated balance.
    #[pallet::storage]
    #[pallet::getter(fn inactive_issuance)]
    #[pallet::whitelist_storage]
    pub type InactiveIssuance<T: Config<I>, I: 'static = ()> =
        StorageValue<_, T::Balance, ValueQuery>;

	#[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
		/// An account was removed whose balance was non-zero but below ExistentialDeposit,
        /// resulting in an outright loss.
        DustLost {
            account: <T as Config<I>>::AccountId,
            amount: T::Balance,
        },
	}

    #[pallet::error]
    pub enum Error<T, I = ()> {}
}

/// Removes a dust account whose balance was non-zero but below `ExistentialDeposit`.
pub struct DustCleaner<T: Config<I>, I: 'static = ()>(
    Option<(<T as Config<I>>::AccountId, NegativeImbalance<T, I>)>,
);

impl<T: Config<I>, I: 'static> Drop for DustCleaner<T, I> {
    fn drop(&mut self) {
        if let Some((who, dust)) = self.0.take() {
            Pallet::<T, I>::deposit_event(Event::DustLost {
                account: who,
                amount: dust.peek(),
            });
            T::DustRemoval::on_unbalanced(dust);
        }
    }
}
