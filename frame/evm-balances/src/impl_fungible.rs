//! Fungible related traits implementation.

use super::*;

impl<T: Config<I>, I: 'static> fungible::Inspect<<T as Config<I>>::AccountId> for Pallet<T, I> {
	type Balance = T::Balance;

	fn total_issuance() -> Self::Balance {
		TotalIssuance::<T, I>::get()
	}

	fn active_issuance() -> Self::Balance {
		TotalIssuance::<T, I>::get().saturating_sub(InactiveIssuance::<T, I>::get())
	}

	fn minimum_balance() -> Self::Balance {
		T::ExistentialDeposit::get()
	}

	fn total_balance(who: &<T as Config<I>>::AccountId) -> Self::Balance {
		Self::account(who).total()
	}

	fn balance(who: &<T as Config<I>>::AccountId) -> Self::Balance {
		Self::account(who).total()
	}

	fn reducible_balance(
		who: &<T as Config<I>>::AccountId,
		preservation: Preservation,
		_force: Fortitude,
	) -> Self::Balance {
		let a = Self::account(who);
		// Liquid balance is what is neither reserved nor locked/frozen.
		let liquid = a.free;
		match preservation {
			Preservation::Expendable => liquid,
			_ => {
				// `must_remain_to_exist` is the part of liquid balance which must remain
				// to keep total over ED.
				let must_remain_to_exist =
					T::ExistentialDeposit::get().saturating_sub(a.total() - liquid);
				liquid.saturating_sub(must_remain_to_exist)
			}
		}
	}

	fn can_deposit(
		who: &<T as Config<I>>::AccountId,
		amount: Self::Balance,
		provenance: Provenance,
	) -> DepositConsequence {
		Self::deposit_consequence(who, amount, provenance)
	}

	fn can_withdraw(
		who: &<T as Config<I>>::AccountId,
		amount: Self::Balance,
	) -> WithdrawConsequence<Self::Balance> {
		Self::withdraw_consequence(who, amount)
	}
}

impl<T: Config<I>, I: 'static> fungible::Unbalanced<<T as Config<I>>::AccountId> for Pallet<T, I> {
	fn handle_dust(dust: fungible::Dust<<T as Config<I>>::AccountId, Self>) {
		T::DustRemoval::on_unbalanced(dust.into_credit());
	}
	fn write_balance(
		who: &<T as Config<I>>::AccountId,
		amount: Self::Balance,
	) -> Result<Option<Self::Balance>, DispatchError> {
		let max_reduction =
			<Self as fungible::Inspect<_>>::reducible_balance(who, Preservation::Expendable, Fortitude::Force);
		let (result, maybe_dust) = Self::mutate_account(who, |account| -> DispatchResult {
			// Make sure the reduction (if there is one) is no more than the maximum allowed.
			let reduction = account.free.saturating_sub(amount);
			ensure!(reduction <= max_reduction, Error::<T, I>::InsufficientBalance);

			account.free = amount;
			Ok(())
		})?;
		result?;
		Ok(maybe_dust)
	}

	fn set_total_issuance(amount: Self::Balance) {
		TotalIssuance::<T, I>::mutate(|t| *t = amount);
	}

	fn deactivate(amount: Self::Balance) {
		InactiveIssuance::<T, I>::mutate(|b| b.saturating_accrue(amount));
	}

	fn reactivate(amount: Self::Balance) {
		InactiveIssuance::<T, I>::mutate(|b| b.saturating_reduce(amount));
	}
}

impl<T: Config<I>, I: 'static> fungible::Mutate<<T as Config<I>>::AccountId> for Pallet<T, I> {
	fn done_mint_into(who: &<T as Config<I>>::AccountId, amount: Self::Balance) {
		Self::deposit_event(Event::Minted { who: who.clone(), amount });
	}

	fn done_burn_from(who: &<T as Config<I>>::AccountId, amount: Self::Balance) {
		Self::deposit_event(Event::Burned { who: who.clone(), amount });
	}

	fn done_shelve(who: &<T as Config<I>>::AccountId, amount: Self::Balance) {
		Self::deposit_event(Event::Suspended { who: who.clone(), amount });
	}

	fn done_restore(who: &<T as Config<I>>::AccountId, amount: Self::Balance) {
		Self::deposit_event(Event::Restored { who: who.clone(), amount });
	}

	fn done_transfer(source: &<T as Config<I>>::AccountId, dest: &<T as Config<I>>::AccountId, amount: Self::Balance) {
		Self::deposit_event(Event::Transfer {
			from: source.clone(),
			to: dest.clone(),
			amount,
		});
	}
}

impl<T: Config<I>, I: 'static> fungible::Balanced<<T as Config<I>>::AccountId> for Pallet<T, I> {
	type OnDropCredit = fungible::DecreaseIssuance<<T as Config<I>>::AccountId, Self>;
	type OnDropDebt = fungible::IncreaseIssuance<<T as Config<I>>::AccountId, Self>;

	fn done_deposit(who: &<T as Config<I>>::AccountId, amount: Self::Balance) {
		Self::deposit_event(Event::Deposit { who: who.clone(), amount });
	}

	fn done_withdraw(who: &<T as Config<I>>::AccountId, amount: Self::Balance) {
		Self::deposit_event(Event::Withdraw { who: who.clone(), amount });
	}

	fn done_issue(amount: Self::Balance) {
		Self::deposit_event(Event::Issued { amount });
	}

	fn done_rescind(amount: Self::Balance) {
		Self::deposit_event(Event::Rescinded { amount });
	}
}
