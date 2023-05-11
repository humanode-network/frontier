//! Unit tests.

use core::str::FromStr;

use frame_support::{assert_ok, assert_noop};
use sp_core::H160;

use crate::{mock::*, *};

#[test]
fn create_account_works() {
    new_test_ext().execute_with(|| {
		let account_id = H160::from_str("1000000000000000000000000000000000000001").unwrap();

		assert_ok!(EvmSystem::create_account(&account_id));
	});
}

#[test]
fn create_account_fails() {
    new_test_ext().execute_with(|| {
		let account_id = H160::from_str("1000000000000000000000000000000000000001").unwrap();
		<FullAccount<Test>>::insert(account_id.clone(), AccountInfo::<_, _>::default());

		assert_noop!(EvmSystem::create_account(&account_id), Error::<Test>::AccountAlreadyExist);
	});
}

#[test]
fn remove_account_works() {
    new_test_ext().execute_with(|| {
		let account_id = H160::from_str("1000000000000000000000000000000000000001").unwrap();
		<FullAccount<Test>>::insert(account_id.clone(), AccountInfo::<_, _>::default());

		assert_ok!(EvmSystem::remove_account(&account_id));
	});
}

#[test]
fn remove_account_fails() {
    new_test_ext().execute_with(|| {
		let account_id = H160::from_str("1000000000000000000000000000000000000001").unwrap();

		assert_noop!(EvmSystem::remove_account(&account_id), Error::<Test>::AccountNotExist);
	});
}

#[test]
fn nonce_update_works() {
    new_test_ext().execute_with(|| {
		let account_id = H160::from_str("1000000000000000000000000000000000000001").unwrap();
		let nonce_before = EvmSystem::account_nonce(&account_id);

		EvmSystem::inc_account_nonce(&account_id);

		let nonce_after = EvmSystem::account_nonce(&account_id);
		assert_eq!(nonce_after, nonce_before + 1);
	});
}
