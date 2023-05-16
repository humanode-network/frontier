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

use frame_support::{
    ensure,
    traits::{
        fungible,
        tokens::{DepositConsequence, WithdrawConsequence},
        Currency, ExistenceRequirement,
        ExistenceRequirement::AllowDeath,
        Get, Imbalance, OnUnbalanced, SignedImbalance, StorageVersion, WithdrawReasons, StoredMap
    },
};
use sp_runtime::{
    traits::{Bounded, CheckedAdd, CheckedSub, MaybeSerializeDeserialize, Zero},
    ArithmeticError, DispatchError, DispatchResult, RuntimeDebug, Saturating,
};
use scale_codec::{Codec, Encode, Decode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_std::{cmp, fmt::Debug, result};

pub mod account_data;
use account_data::{AccountData, Reasons};

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
		/// An account was created with some free balance.
        Endowed {
            account: <T as Config<I>>::AccountId,
            free_balance: T::Balance,
        },
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

impl<T: Config<I>, I: 'static> Pallet<T, I> {
	/// Get the free balance of an account.
	pub fn free_balance(who: impl sp_std::borrow::Borrow<<T as Config<I>>::AccountId>) -> T::Balance {
		Self::account(who.borrow()).free
	}

	/// Get the balance of an account that can be used for transfers, reservations, or any other
	/// non-locking, non-transaction-fee activity. Will be at most `free_balance`.
	pub fn usable_balance(who: impl sp_std::borrow::Borrow<<T as Config<I>>::AccountId>) -> T::Balance {
		Self::account(who.borrow()).usable(Reasons::Misc)
	}

	/// Get the balance of an account that can be used for paying transaction fees (not tipping,
	/// or any other kind of fees, though). Will be at most `free_balance`.
	pub fn usable_balance_for_fees(who: impl sp_std::borrow::Borrow<<T as Config<I>>::AccountId>) -> T::Balance {
		Self::account(who.borrow()).usable(Reasons::Fee)
	}

	/// Get the reserved balance of an account.
	pub fn reserved_balance(who: impl sp_std::borrow::Borrow<<T as Config<I>>::AccountId>) -> T::Balance {
		Self::account(who.borrow()).reserved
	}

    /// Get all balance information for an account.
    fn account(who: &<T as Config<I>>::AccountId) -> AccountData<T::Balance> {
        T::AccountStore::get(who)
    }

    /// Mutate an account to some new value, or delete it entirely with `None`. Will enforce
    /// `ExistentialDeposit` law, annulling the account as needed. This will do nothing if the
    /// result of `f` is an `Err`.
    ///
    /// NOTE: Doesn't do any preparatory work for creating a new account, so should only be used
    /// when it is known that the account already exists.
    ///
    /// NOTE: LOW-LEVEL: This will not attempt to maintain total issuance. It is expected that
    /// the caller will do this.
    fn try_mutate_account<R, E: From<DispatchError>>(
        who: &<T as Config<I>>::AccountId,
        f: impl FnOnce(&mut AccountData<T::Balance>, bool) -> Result<R, E>,
    ) -> Result<R, E> {
        Self::try_mutate_account_with_dust(who, f).map(|(result, dust_cleaner)| {
            drop(dust_cleaner);
            result
        })
    }

    /// Mutate an account to some new value, or delete it entirely with `None`. Will enforce
    /// `ExistentialDeposit` law, annulling the account as needed. This will do nothing if the
    /// result of `f` is an `Err`.
    ///
    /// It returns both the result from the closure, and an optional `DustCleaner` instance which
    /// should be dropped once it is known that all nested mutates that could affect storage items
    /// what the dust handler touches have completed.
    ///
    /// NOTE: Doesn't do any preparatory work for creating a new account, so should only be used
    /// when it is known that the account already exists.
    ///
    /// NOTE: LOW-LEVEL: This will not attempt to maintain total issuance. It is expected that
    /// the caller will do this.
    fn try_mutate_account_with_dust<R, E: From<DispatchError>>(
        who: &<T as Config<I>>::AccountId,
        f: impl FnOnce(&mut AccountData<T::Balance>, bool) -> Result<R, E>,
    ) -> Result<(R, DustCleaner<T, I>), E> {
        let result = T::AccountStore::try_mutate_exists(who, |maybe_account| {
            let is_new = maybe_account.is_none();
            let mut account = maybe_account.take().unwrap_or_default();
            f(&mut account, is_new).map(move |result| {
                let maybe_endowed = if is_new { Some(account.free) } else { None };
                let maybe_account_maybe_dust = Self::post_mutation(who, account);
                *maybe_account = maybe_account_maybe_dust.0;
                (maybe_endowed, maybe_account_maybe_dust.1, result)
            })
        });
        result.map(|(maybe_endowed, maybe_dust, result)| {
            if let Some(endowed) = maybe_endowed {
                Self::deposit_event(Event::Endowed {
                    account: who.clone(),
                    free_balance: endowed,
                });
            }
            let dust_cleaner = DustCleaner(maybe_dust.map(|dust| (who.clone(), dust)));
            (result, dust_cleaner)
        })
    }

    /// Handles any steps needed after mutating an account.
    ///
    /// This includes `DustRemoval` unbalancing, in the case than the `new` account's total balance
    /// is non-zero but below ED.
    ///
    /// Returns two values:
    /// - `Some` containing the the `new` account, iff the account has sufficient balance.
    /// - `Some` containing the dust to be dropped, iff some dust should be dropped.
    fn post_mutation(
        _who: &<T as Config<I>>::AccountId,
        new: AccountData<T::Balance>,
    ) -> (
        Option<AccountData<T::Balance>>,
        Option<NegativeImbalance<T, I>>,
    ) {
        let total = new.total();
        if total < T::ExistentialDeposit::get() {
            if total.is_zero() {
                (None, None)
            } else {
                (None, Some(NegativeImbalance::new(total)))
            }
        } else {
            (Some(new), None)
        }
    }

    fn deposit_consequence(
        _who: &<T as Config<I>>::AccountId,
        amount: T::Balance,
        account: &AccountData<T::Balance>,
        mint: bool,
    ) -> DepositConsequence {
        if amount.is_zero() {
            return DepositConsequence::Success;
        }

        if mint && TotalIssuance::<T, I>::get().checked_add(&amount).is_none() {
            return DepositConsequence::Overflow;
        }

        let new_total_balance = match account.total().checked_add(&amount) {
            Some(x) => x,
            None => return DepositConsequence::Overflow,
        };

        if new_total_balance < T::ExistentialDeposit::get() {
            return DepositConsequence::BelowMinimum;
        }

        // NOTE: We assume that we are a provider, so don't need to do any checks in the
        // case of account creation.

        DepositConsequence::Success
    }

    fn withdraw_consequence(
        _who: &<T as Config<I>>::AccountId,
        amount: T::Balance,
        account: &AccountData<T::Balance>,
    ) -> WithdrawConsequence<T::Balance> {
        if amount.is_zero() {
            return WithdrawConsequence::Success;
        }

        if TotalIssuance::<T, I>::get().checked_sub(&amount).is_none() {
            return WithdrawConsequence::Underflow;
        }

        let new_total_balance = match account.total().checked_sub(&amount) {
            Some(x) => x,
            None => return WithdrawConsequence::NoFunds,
        };

        // Provider restriction - total account balance cannot be reduced to zero if it cannot
        // sustain the loss of a provider reference.
        // NOTE: This assumes that the pallet is a provider (which is true). Is this ever changes,
        // then this will need to adapt accordingly.
        let ed = T::ExistentialDeposit::get();
        let success = if new_total_balance < ed {
            // ATTENTION. CHECK.
            // if frame_system::Pallet::<T>::can_dec_provider(who) {
            //     WithdrawConsequence::ReducedToZero(new_total_balance)
            // } else {
            //     return WithdrawConsequence::WouldDie;
            // }
            WithdrawConsequence::ReducedToZero(new_total_balance)
        } else {
            WithdrawConsequence::Success
        };

        // Enough free funds to have them be reduced.
        let new_free_balance = match account.free.checked_sub(&amount) {
            Some(b) => b,
            None => return WithdrawConsequence::NoFunds,
        };

        // Eventual free funds must be no less than the frozen balance.
        let min_balance = account.frozen(Reasons::All);
        if new_free_balance < min_balance {
            return WithdrawConsequence::Frozen;
        }

        success
    }
}
