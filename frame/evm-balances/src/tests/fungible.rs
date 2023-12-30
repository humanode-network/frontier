//! Tests regarding the functionality of the fungible trait set implementations.

use frame_support::{assert_noop, assert_ok, traits::fungible::Inspect};
use sp_core::H160;
use sp_runtime::TokenError;
use sp_std::str::FromStr;

use crate::{mock::*, *};

#[test]
fn total_issuance_works() {
	new_test_ext().execute_with_ext(|_| {
		// Check the total issuance value.
		assert_eq!(EvmBalances::total_issuance(), 2 * INIT_BALANCE);
	});
}

#[test]
fn active_issuance_works() {
	new_test_ext().execute_with_ext(|_| {
		// Check the active issuance value.
		assert_eq!(EvmBalances::active_issuance(), 2 * INIT_BALANCE);
	});
}

#[test]
fn minimum_balance_works() {
	new_test_ext().execute_with_ext(|_| {
		// Check the minimum balance value.
		assert_eq!(EvmBalances::minimum_balance(), 1);
	});
}

#[test]
fn total_balance_works() {
	new_test_ext().execute_with_ext(|_| {
		// Check the total balance value.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);
	});
}

#[test]
fn balance_works() {
	new_test_ext().execute_with_ext(|_| {
		// Check the balance value.
		assert_eq!(EvmBalances::balance(&alice()), INIT_BALANCE);
	});
}

#[test]
fn reducable_balance_works() {
	new_test_ext().execute_with_ext(|_| {
		// Check the reducable balance value in `Expendable` case.
		assert_eq!(
			EvmBalances::reducible_balance(&alice(), Preservation::Expendable, Fortitude::Polite),
			INIT_BALANCE
		);

		// Check the reducable balance value in `Preserve` case.
		assert_eq!(
			EvmBalances::reducible_balance(&alice(), Preservation::Preserve, Fortitude::Polite),
			INIT_BALANCE - 1
		);

		// Check the reducable balance value in `Protect` case.
		assert_eq!(
			EvmBalances::reducible_balance(&alice(), Preservation::Protect, Fortitude::Polite),
			INIT_BALANCE - 1
		);
	});
}

#[test]
fn can_deposit_works_success() {
	new_test_ext().execute_with_ext(|_| {
		assert_eq!(
			EvmBalances::can_deposit(&alice(), 10, Provenance::Minted),
			DepositConsequence::Success
		);
	});
}

#[test]
fn can_deposit_works_overflow() {
	new_test_ext().execute_with_ext(|_| {
		assert_eq!(
			EvmBalances::can_deposit(&alice(), u64::MAX, Provenance::Minted),
			DepositConsequence::Overflow
		);
	});
}

#[test]
fn can_withdraw_works_success() {
	new_test_ext().execute_with_ext(|_| {
		assert_eq!(
			EvmBalances::can_withdraw(&alice(), 10),
			WithdrawConsequence::Success
		);
	});
}

#[test]
fn can_withdraw_works_underflow() {
	new_test_ext().execute_with_ext(|_| {
		assert_eq!(
			EvmBalances::can_withdraw(&alice(), u64::MAX),
			WithdrawConsequence::Underflow
		);
	});
}

#[test]
fn can_withdraw_works_balance_low() {
	new_test_ext().execute_with_ext(|_| {
		assert_eq!(
			EvmBalances::can_withdraw(&alice(), INIT_BALANCE + 1),
			WithdrawConsequence::BalanceLow
		);
	});
}

#[test]
fn can_withdraw_works_reduced_to_zero() {
	new_test_ext().execute_with_ext(|_| {
		assert_eq!(
			EvmBalances::can_withdraw(&alice(), INIT_BALANCE),
			WithdrawConsequence::ReducedToZero(0)
		);
	});
}
