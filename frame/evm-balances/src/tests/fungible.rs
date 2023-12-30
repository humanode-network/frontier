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
