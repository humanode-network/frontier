// SPDX-License-Identifier: Apache-2.0
// This file is part of Frontier.
//
// Copyright (c) 2022 Parity Technologies (UK) Ltd.
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

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{pallet_prelude::MaybeSerializeDeserialize, traits::{Currency, ExistenceRequirement::AllowDeath, TryDrop, Get, Imbalance, WithdrawReasons, OnUnbalanced, ExistenceRequirement, SignedImbalance, tokens::{WithdrawConsequence, DepositConsequence}, fungible}, ensure};
use scale_codec::{Encode, Codec, Decode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{RuntimeDebug, Saturating, traits::{Zero, Bounded, CheckedSub, CheckedAdd}, DispatchError, DispatchResult, ArithmeticError};
use sp_std::cmp;

pub use pallet::*;
pub use self::imbalances::{NegativeImbalance, PositiveImbalance};

/// Simplified reasons for withdrawing balance.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum Reasons {
	/// Paying system transaction fees.
	Fee = 0,
	/// Any reason other than paying system transaction fees.
	Misc = 1,
	/// Any reason at all.
	All = 2,
}

impl From<WithdrawReasons> for Reasons {
	fn from(r: WithdrawReasons) -> Reasons {
		if r == WithdrawReasons::TRANSACTION_PAYMENT {
			Reasons::Fee
		} else if r.contains(WithdrawReasons::TRANSACTION_PAYMENT) {
			Reasons::All
		} else {
			Reasons::Misc
		}
	}
}

/// All balance information for an account.
#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct AccountData<Balance> {
	/// Non-reserved part of the balance. There may still be restrictions on this, but it is the
	/// total pool what may in principle be transferred, reserved and used for tipping.
	///
	/// This is the only balance that matters in terms of most operations on tokens. It
	/// alone is used to determine the balance when in the contract execution environment.
	pub free: Balance,
	/// Balance which is reserved and may not be used at all.
	///
	/// This can still get slashed, but gets slashed last of all.
	///
	/// This balance is a 'reserve' balance that other subsystems use in order to set aside tokens
	/// that are still 'owned' by the account holder, but which are suspendable.
	/// This includes named reserve and unnamed reserve.
	pub reserved: Balance,
	/// The amount that `free` may not drop below when withdrawing for *anything except transaction
	/// fee payment*.
	pub misc_frozen: Balance,
	/// The amount that `free` may not drop below when withdrawing specifically for transaction
	/// fee payment.
	pub fee_frozen: Balance,
}

impl<Balance: Saturating + Copy + Ord> AccountData<Balance> {
	/// How much this account's balance can be reduced for the given `reasons`.
	fn usable(&self, reasons: Reasons) -> Balance {
		self.free.saturating_sub(self.frozen(reasons))
	}
	/// The amount that this account's free balance may not be reduced beyond for the given
	/// `reasons`.
	fn frozen(&self, reasons: Reasons) -> Balance {
		match reasons {
			Reasons::All => self.misc_frozen.max(self.fee_frozen),
			Reasons::Misc => self.misc_frozen,
			Reasons::Fee => self.fee_frozen,
		}
	}
	/// The total balance in this account including any that is reserved and ignoring any frozen.
	fn total(&self) -> Balance {
		self.free.saturating_add(self.reserved)
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::{MaybeDisplay, AtLeast32BitUnsigned};
	use sp_std::fmt::Debug;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>>
			+ IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The user account identifier type for the runtime.
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
			+ sp_runtime::FixedPointOperand;

		/// The minimum amount required to keep an account open.
		#[pallet::constant]
		type ExistentialDeposit: Get<Self::Balance>;

		/// Handler for the unbalanced reduction when removing a dust account.
		type DustRemoval: OnUnbalanced<NegativeImbalance<Self>>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// An account was created with some free balance.
		Endowed { account: <T as Config>::AccountId, free_balance: T::Balance },
		/// An account was removed whose balance was non-zero but below ExistentialDeposit,
		/// resulting in an outright loss.
		DustLost { account: <T as Config>::AccountId, amount: T::Balance },
		/// Transfer succeeded.
		Transfer { from: <T as Config>::AccountId, to: <T as Config>::AccountId, amount: T::Balance },
		/// A balance was set by root.
		BalanceSet { who: <T as Config>::AccountId, free: T::Balance, reserved: T::Balance },
		/// Some balance was reserved (moved from free to reserved).
		Reserved { who: <T as Config>::AccountId, amount: T::Balance },
		/// Some amount was deposited (e.g. for transaction fees).
		Deposit { who: <T as Config>::AccountId, amount: T::Balance },
		/// Some amount was withdrawn from the account (e.g. for transaction fees).
		Withdraw { who: <T as Config>::AccountId, amount: T::Balance },
		/// Some amount was removed from the account (e.g. for misbehavior).
		Slashed { who: <T as Config>::AccountId, amount: T::Balance },
	}

	/// The total units issued in the system.
	#[pallet::storage]
	#[pallet::getter(fn total_issuance)]
	#[pallet::whitelist_storage]
	pub type TotalIssuance<T: Config> = StorageValue<_, T::Balance, ValueQuery>;

	/// The total units of outstanding deactivated balance in the system.
	#[pallet::storage]
	#[pallet::getter(fn inactive_issuance)]
	#[pallet::whitelist_storage]
	pub type InactiveIssuance<T: Config> =
		StorageValue<_, T::Balance, ValueQuery>;

	/// The full account information for a particular account ID.
	#[pallet::storage]
	#[pallet::getter(fn account_store)]
	pub type AccountStore<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		<T as Config>::AccountId,
		AccountData<T::Balance>,
		ValueQuery,
	>;

	#[pallet::error]
	pub enum Error<T> {
		/// Account liquidity restrictions prevent withdrawal
		LiquidityRestrictions,
		/// Balance too low to send value.
		InsufficientBalance,
		/// Value too low to create account due to existential deposit
		ExistentialDeposit,
		/// Transfer/payment would kill account
		KeepAlive,
		/// A vesting schedule already exists for this account
		ExistingVestingSchedule,
		/// Beneficiary account must pre-exist
		DeadAccount,
		/// Number of named reserves exceed MaxReserves
		TooManyReserves,
	}
}

pub struct DustCleaner<T: Config>(
	Option<(<T as Config>::AccountId, NegativeImbalance<T>)>,
);

impl<T: Config> Drop for DustCleaner<T> {
	fn drop(&mut self) {
		if let Some((who, dust)) = self.0.take() {
			Pallet::<T>::deposit_event(Event::DustLost { account: who, amount: dust.peek() });
			T::DustRemoval::on_unbalanced(dust);
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Get both the free and reserved balances of an account.
	fn account(who: &<T as Config>::AccountId) -> AccountData<T::Balance> {
		<AccountStore<T>>::get(who)
	}

	fn post_mutation(
		_who: &<T as Config>::AccountId,
		new: AccountData<T::Balance>,
	) -> (Option<AccountData<T::Balance>>, Option<NegativeImbalance<T>>) {
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

	fn try_mutate_account<R, E: From<DispatchError>>(
		who: &<T as Config>::AccountId,
		f: impl FnOnce(&mut AccountData<T::Balance>, bool) -> Result<R, E>,
	) -> Result<R, E> {
		Self::try_mutate_account_with_dust(who, f).map(|(result, dust_cleaner)| {
			drop(dust_cleaner);
			result
		})
	}

	fn try_mutate_account_with_dust<R, E: From<DispatchError>>(
		who: &<T as Config>::AccountId,
		f: impl FnOnce(&mut AccountData<T::Balance>, bool) -> Result<R, E>,
	) -> Result<(R, DustCleaner<T>), E> {
		let result = <AccountStore<T>>::try_mutate_exists(who, |maybe_account| {
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
				Self::deposit_event(Event::Endowed { account: who.clone(), free_balance: endowed });
			}
			let dust_cleaner = DustCleaner(maybe_dust.map(|dust| (who.clone(), dust)));
			(result, dust_cleaner)
		})
	}

	fn deposit_consequence(
		_who: &<T as Config>::AccountId,
		amount: T::Balance,
		account: &AccountData<T::Balance>,
		mint: bool,
	) -> DepositConsequence {
		if amount.is_zero() {
			return DepositConsequence::Success
		}

		if mint && TotalIssuance::<T>::get().checked_add(&amount).is_none() {
			return DepositConsequence::Overflow
		}

		let new_total_balance = match account.total().checked_add(&amount) {
			Some(x) => x,
			None => return DepositConsequence::Overflow,
		};

		if new_total_balance < T::ExistentialDeposit::get() {
			return DepositConsequence::BelowMinimum
		}

		// NOTE: We assume that we are a provider, so don't need to do any checks in the
		// case of account creation.

		DepositConsequence::Success
	}

	fn withdraw_consequence(
		who: &<T as Config>::AccountId,
		amount: T::Balance,
		account: &AccountData<T::Balance>,
	) -> WithdrawConsequence<T::Balance> {
		if amount.is_zero() {
			return WithdrawConsequence::Success
		}

		if TotalIssuance::<T>::get().checked_sub(&amount).is_none() {
			return WithdrawConsequence::Underflow
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
			WithdrawConsequence::ReducedToZero(new_total_balance)
			// if frame_system::Pallet::<T>::can_dec_provider(who) {
			// 	WithdrawConsequence::ReducedToZero(new_total_balance)
			// } else {
			// 	return WithdrawConsequence::WouldDie
			// }
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
			return WithdrawConsequence::Frozen
		}

		success
	}
}

// wrapping these imbalances in a private module is necessary to ensure absolute privacy
// of the inner member.
mod imbalances {
	use super::{Config, Imbalance, RuntimeDebug, TryDrop};
	use frame_support::traits::{SameOrOther};
	use sp_runtime::{Saturating, traits::Zero};
	use sp_std::{result, mem};

	/// Opaque, move-only struct with private fields that serves as a token denoting that
	/// funds have been created without any equal and opposite accounting.
	#[must_use]
	#[derive(RuntimeDebug, PartialEq, Eq)]
	pub struct PositiveImbalance<T: Config>(T::Balance);

	impl<T: Config> PositiveImbalance<T> {
		/// Create a new positive imbalance from a balance.
		pub fn new(amount: T::Balance) -> Self {
			PositiveImbalance(amount)
		}
	}

	/// Opaque, move-only struct with private fields that serves as a token denoting that
	/// funds have been destroyed without any equal and opposite accounting.
	#[must_use]
	#[derive(RuntimeDebug, PartialEq, Eq)]
	pub struct NegativeImbalance<T: Config>(T::Balance);

	impl<T: Config> NegativeImbalance<T> {
		/// Create a new negative imbalance from a balance.
		pub fn new(amount: T::Balance) -> Self {
			NegativeImbalance(amount)
		}
	}

	impl<T: Config> TryDrop for PositiveImbalance<T> {
		fn try_drop(self) -> result::Result<(), Self> {
			self.drop_zero()
		}
	}

	impl<T: Config> Default for PositiveImbalance<T> {
		fn default() -> Self {
			Self::zero()
		}
	}

	impl<T: Config> Imbalance<T::Balance> for PositiveImbalance<T> {
		type Opposite = NegativeImbalance<T>;

		fn zero() -> Self {
			Self(Zero::zero())
		}
		fn drop_zero(self) -> result::Result<(), Self> {
			if self.0.is_zero() {
				Ok(())
			} else {
				Err(self)
			}
		}
		fn split(self, amount: T::Balance) -> (Self, Self) {
			let first = self.0.min(amount);
			let second = self.0 - first;

			mem::forget(self);
			(Self(first), Self(second))
		}
		fn merge(mut self, other: Self) -> Self {
			self.0 = self.0.saturating_add(other.0);
			mem::forget(other);

			self
		}
		fn subsume(&mut self, other: Self) {
			self.0 = self.0.saturating_add(other.0);
			mem::forget(other);
		}
		fn offset(self, other: Self::Opposite) -> SameOrOther<Self, Self::Opposite> {
			let (a, b) = (self.0, other.0);
			mem::forget((self, other));

			if a > b {
				SameOrOther::Same(Self(a - b))
			} else if b > a {
				SameOrOther::Other(NegativeImbalance::new(b - a))
			} else {
				SameOrOther::None
			}
		}
		fn peek(&self) -> T::Balance {
			self.0
		}
	}

	impl<T: Config> TryDrop for NegativeImbalance<T> {
		fn try_drop(self) -> result::Result<(), Self> {
			self.drop_zero()
		}
	}

	impl<T: Config> Default for NegativeImbalance<T> {
		fn default() -> Self {
			Self::zero()
		}
	}

	impl<T: Config> Imbalance<T::Balance> for NegativeImbalance<T> {
		type Opposite = PositiveImbalance<T>;

		fn zero() -> Self {
			Self(Zero::zero())
		}
		fn drop_zero(self) -> result::Result<(), Self> {
			if self.0.is_zero() {
				Ok(())
			} else {
				Err(self)
			}
		}
		fn split(self, amount: T::Balance) -> (Self, Self) {
			let first = self.0.min(amount);
			let second = self.0 - first;

			mem::forget(self);
			(Self(first), Self(second))
		}
		fn merge(mut self, other: Self) -> Self {
			self.0 = self.0.saturating_add(other.0);
			mem::forget(other);

			self
		}
		fn subsume(&mut self, other: Self) {
			self.0 = self.0.saturating_add(other.0);
			mem::forget(other);
		}
		fn offset(self, other: Self::Opposite) -> SameOrOther<Self, Self::Opposite> {
			let (a, b) = (self.0, other.0);
			mem::forget((self, other));

			if a > b {
				SameOrOther::Same(Self(a - b))
			} else if b > a {
				SameOrOther::Other(PositiveImbalance::new(b - a))
			} else {
				SameOrOther::None
			}
		}
		fn peek(&self) -> T::Balance {
			self.0
		}
	}

	impl<T: Config> Drop for PositiveImbalance<T> {
		/// Basic drop handler will just square up the total issuance.
		fn drop(&mut self) {
			<super::TotalIssuance<T>>::mutate(|v| *v = v.saturating_add(self.0));
		}
	}

	impl<T: Config> Drop for NegativeImbalance<T> {
		/// Basic drop handler will just square up the total issuance.
		fn drop(&mut self) {
			<super::TotalIssuance<T>>::mutate(|v| *v = v.saturating_sub(self.0));
		}
	}
}

impl<T: Config> Currency<<T as Config>::AccountId> for Pallet<T> {
    type Balance = T::Balance;

    type PositiveImbalance = imbalances::PositiveImbalance<T>;

    type NegativeImbalance = imbalances::NegativeImbalance<T>;

    fn total_balance(who: &<T as Config>::AccountId) -> Self::Balance {
        Self::account(who).total()
    }

    fn can_slash(who: &<T as Config>::AccountId, value: Self::Balance) -> bool {
        if value.is_zero() {
			return true
		}
		Self::free_balance(who) >= value
    }

    fn total_issuance() -> Self::Balance {
        TotalIssuance::<T>::get()
    }

	fn active_issuance() -> Self::Balance {
		<Self as fungible::Inspect<<T as Config>::AccountId>>::active_issuance()
	}

	fn deactivate(amount: Self::Balance) {
		InactiveIssuance::<T>::mutate(|b| b.saturating_accrue(amount));
	}

	fn reactivate(amount: Self::Balance) {
		InactiveIssuance::<T>::mutate(|b| b.saturating_reduce(amount));
	}

    fn minimum_balance() -> Self::Balance {
        T::ExistentialDeposit::get()
    }

    fn burn(mut amount: Self::Balance) -> Self::PositiveImbalance {
		if amount.is_zero() {
			return Self::PositiveImbalance::zero()
		}
		<TotalIssuance<T>>::mutate(|issued| {
			*issued = issued.checked_sub(&amount).unwrap_or_else(|| {
				amount = *issued;
				Zero::zero()
			});
		});
		Self::PositiveImbalance::new(amount)
    }

    fn issue(mut amount: Self::Balance) -> Self::NegativeImbalance {
        if amount.is_zero() {
			return Self::NegativeImbalance::zero()
		}
		<TotalIssuance<T>>::mutate(|issued| {
			*issued = issued.checked_add(&amount).unwrap_or_else(|| {
				amount = Self::Balance::max_value() - *issued;
				Self::Balance::max_value()
			})
		});
		Self::NegativeImbalance::new(amount)
    }

    fn free_balance(who: &<T as Config>::AccountId) -> Self::Balance {
        Self::account(who).free
    }

    fn ensure_can_withdraw(
		who: &<T as Config>::AccountId,
		amount: Self::Balance,
		reasons: frame_support::traits::WithdrawReasons,
		new_balance: Self::Balance,
	) -> frame_support::pallet_prelude::DispatchResult {
        if amount.is_zero() {
			return Ok(())
		}
		let min_balance = Self::account(who).frozen(reasons.into());
		ensure!(new_balance >= min_balance, Error::<T>::LiquidityRestrictions);
		Ok(())
    }

    fn transfer(
		source: &<T as Config>::AccountId,
		dest: &<T as Config>::AccountId,
		value: Self::Balance,
		existence_requirement: frame_support::traits::ExistenceRequirement,
	) -> DispatchResult {
        if value.is_zero() || source == dest {
			return Ok(())
		}

		Self::try_mutate_account_with_dust(
			dest,
			|to_account, _| -> Result<DustCleaner<T>, DispatchError> {
				Self::try_mutate_account_with_dust(
					source,
					|from_account, _| -> DispatchResult {
						from_account.free = from_account
							.free
							.checked_sub(&value)
							.ok_or(Error::<T>::InsufficientBalance)?;

						// NOTE: total stake being stored in the same type means that this could
						// never overflow but better to be safe than sorry.
						to_account.free =
							to_account.free.checked_add(&value).ok_or(ArithmeticError::Overflow)?;

						let ed = T::ExistentialDeposit::get();
						ensure!(to_account.total() >= ed, Error::<T>::ExistentialDeposit);

						Self::ensure_can_withdraw(
							source,
							value,
							WithdrawReasons::TRANSFER,
							from_account.free,
						)
						.map_err(|_| Error::<T>::LiquidityRestrictions)?;

						// TODO: This is over-conservative. There may now be other providers, and
						// this pallet may not even be a provider.
						let allow_death = existence_requirement == ExistenceRequirement::AllowDeath;
						// let allow_death =
						// 	allow_death && system::Pallet::<T>::can_dec_provider(source);
						ensure!(
							allow_death || from_account.total() >= ed,
							Error::<T>::KeepAlive
						);

						Ok(())
					},
				)
				.map(|(_, maybe_dust_cleaner)| maybe_dust_cleaner)
			},
		)?;

		// Emit transfer event.
		Self::deposit_event(Event::Transfer {
			from: source.clone(),
			to: dest.clone(),
			amount: value,
		});

		Ok(())
    }

    fn slash(who: &<T as Config>::AccountId, value: Self::Balance) -> (Self::NegativeImbalance, Self::Balance) {
        if value.is_zero() {
			return (NegativeImbalance::zero(), Zero::zero())
		}
		if Self::total_balance(who).is_zero() {
			return (NegativeImbalance::zero(), value)
		}

		for attempt in 0..2 {
			match Self::try_mutate_account(
				who,
				|account,
				 _is_new|
				 -> Result<(Self::NegativeImbalance, Self::Balance), DispatchError> {
					// Best value is the most amount we can slash following liveness rules.
					let best_value = match attempt {
						// First attempt we try to slash the full amount, and see if liveness issues
						// happen.
						0 => value,
						// If acting as a critical provider (i.e. first attempt failed), then slash
						// as much as possible while leaving at least at ED.
						_ => value.min(
							(account.free + account.reserved)
								.saturating_sub(T::ExistentialDeposit::get()),
						),
					};

					let free_slash = cmp::min(account.free, best_value);
					account.free -= free_slash; // Safe because of above check
					let remaining_slash = best_value - free_slash; // Safe because of above check

					if !remaining_slash.is_zero() {
						// If we have remaining slash, take it from reserved balance.
						let reserved_slash = cmp::min(account.reserved, remaining_slash);
						account.reserved -= reserved_slash; // Safe because of above check
						Ok((
							NegativeImbalance::new(free_slash + reserved_slash),
							value - free_slash - reserved_slash, /* Safe because value is gt or
							                                      * eq total slashed */
						))
					} else {
						// Else we are done!
						Ok((
							NegativeImbalance::new(free_slash),
							value - free_slash, // Safe because value is gt or eq to total slashed
						))
					}
				},
			) {
				Ok((imbalance, not_slashed)) => {
					Self::deposit_event(Event::Slashed {
						who: who.clone(),
						amount: value.saturating_sub(not_slashed),
					});
					return (imbalance, not_slashed)
				},
				Err(_) => (),
			}
		}

		// Should never get here. But we'll be defensive anyway.
		(Self::NegativeImbalance::zero(), value)
    }

    fn deposit_into_existing(
		who: &<T as Config>::AccountId,
		value: Self::Balance,
	) -> Result<Self::PositiveImbalance, sp_runtime::DispatchError> {
        if value.is_zero() {
			return Ok(PositiveImbalance::zero())
		}

		Self::try_mutate_account(
			who,
			|account, is_new| -> Result<Self::PositiveImbalance, DispatchError> {
				ensure!(!is_new, Error::<T>::DeadAccount);
				account.free = account.free.checked_add(&value).ok_or(ArithmeticError::Overflow)?;
				Self::deposit_event(Event::Deposit { who: who.clone(), amount: value });
				Ok(PositiveImbalance::new(value))
			},
		)
    }

    fn deposit_creating(who: &<T as Config>::AccountId, value: Self::Balance) -> Self::PositiveImbalance {
        if value.is_zero() {
			return Self::PositiveImbalance::zero()
		}

		Self::try_mutate_account(
			who,
			|account, is_new| -> Result<Self::PositiveImbalance, DispatchError> {
				let ed = T::ExistentialDeposit::get();
				ensure!(value >= ed || !is_new, Error::<T>::ExistentialDeposit);

				// defensive only: overflow should never happen, however in case it does, then this
				// operation is a no-op.
				account.free = match account.free.checked_add(&value) {
					Some(x) => x,
					None => return Ok(Self::PositiveImbalance::zero()),
				};

				Self::deposit_event(Event::Deposit { who: who.clone(), amount: value });
				Ok(PositiveImbalance::new(value))
			},
		)
		.unwrap_or_else(|_| Self::PositiveImbalance::zero())
    }

    fn withdraw(
		who: &<T as Config>::AccountId,
		value: Self::Balance,
		reasons: frame_support::traits::WithdrawReasons,
		liveness: frame_support::traits::ExistenceRequirement,
	) -> Result<Self::NegativeImbalance, sp_runtime::DispatchError> {
        if value.is_zero() {
			return Ok(NegativeImbalance::zero())
		}

		Self::try_mutate_account(
			who,
			|account, _| -> Result<Self::NegativeImbalance, DispatchError> {
				let new_free_account =
					account.free.checked_sub(&value).ok_or(Error::<T>::InsufficientBalance)?;

				// bail if we need to keep the account alive and this would kill it.
				let ed = T::ExistentialDeposit::get();
				let would_be_dead = new_free_account + account.reserved < ed;
				let would_kill = would_be_dead && account.free + account.reserved >= ed;
				ensure!(liveness == AllowDeath || !would_kill, Error::<T>::KeepAlive);

				Self::ensure_can_withdraw(who, value, reasons, new_free_account)?;

				account.free = new_free_account;

				Self::deposit_event(Event::Withdraw { who: who.clone(), amount: value });
				Ok(NegativeImbalance::new(value))
			},
		)
    }

    fn make_free_balance_be(
		who: &<T as Config>::AccountId,
		balance: Self::Balance,
	) -> frame_support::traits::SignedImbalance<Self::Balance, Self::PositiveImbalance> {
        Self::try_mutate_account(
			who,
			|account,
			 is_new|
			 -> Result<SignedImbalance<Self::Balance, Self::PositiveImbalance>, DispatchError> {
				let ed = T::ExistentialDeposit::get();
				let total = balance.saturating_add(account.reserved);
				// If we're attempting to set an existing account to less than ED, then
				// bypass the entire operation. It's a no-op if you follow it through, but
				// since this is an instance where we might account for a negative imbalance
				// (in the dust cleaner of set_account) before we account for its actual
				// equal and opposite cause (returned as an Imbalance), then in the
				// instance that there's no other accounts on the system at all, we might
				// underflow the issuance and our arithmetic will be off.
				ensure!(total >= ed || !is_new, Error::<T>::ExistentialDeposit);

				let imbalance = if account.free <= balance {
					SignedImbalance::Positive(PositiveImbalance::new(balance - account.free))
				} else {
					SignedImbalance::Negative(NegativeImbalance::new(account.free - balance))
				};
				account.free = balance;
				Self::deposit_event(Event::BalanceSet {
					who: who.clone(),
					free: account.free,
					reserved: account.reserved,
				});
				Ok(imbalance)
			},
		)
		.unwrap_or_else(|_| SignedImbalance::Positive(Self::PositiveImbalance::zero()))
    }
}

impl<T: Config> fungible::Inspect<<T as Config>::AccountId> for Pallet<T> {
	type Balance = T::Balance;

	fn total_issuance() -> Self::Balance {
		TotalIssuance::<T>::get()
	}
	fn active_issuance() -> Self::Balance {
		TotalIssuance::<T>::get().saturating_sub(InactiveIssuance::<T>::get())
	}
	fn minimum_balance() -> Self::Balance {
		T::ExistentialDeposit::get()
	}
	fn balance(who: &<T as Config>::AccountId) -> Self::Balance {
		Self::account(who).total()
	}
	fn reducible_balance(who: &<T as Config>::AccountId, keep_alive: bool) -> Self::Balance {
		let a = Self::account(who);
		// Liquid balance is what is neither reserved nor locked/frozen.
		let liquid = a.free.saturating_sub(a.fee_frozen.max(a.misc_frozen));
		// if frame_system::Pallet::<T>::can_dec_provider(who) && !keep_alive {
		if !keep_alive {
			liquid
		} else {
			// `must_remain_to_exist` is the part of liquid balance which must remain to keep total
			// over ED.
			let must_remain_to_exist =
				T::ExistentialDeposit::get().saturating_sub(a.total() - liquid);
			liquid.saturating_sub(must_remain_to_exist)
		}
	}
	fn can_deposit(who: &<T as Config>::AccountId, amount: Self::Balance, mint: bool) -> DepositConsequence {
		Self::deposit_consequence(who, amount, &Self::account(who), mint)
	}
	fn can_withdraw(
		who: &<T as Config>::AccountId,
		amount: Self::Balance,
	) -> WithdrawConsequence<Self::Balance> {
		Self::withdraw_consequence(who, amount, &Self::account(who))
	}
}
