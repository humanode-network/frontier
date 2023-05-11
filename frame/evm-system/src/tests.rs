//! Unit tests.

use core::str::FromStr;

use frame_support::{assert_ok, assert_noop};
use mockall::predicate;
use sp_core::H160;

use crate::{mock::*, *};

#[test]
fn create_account_works() {
    new_test_ext().execute_with_ext(|_| {
		// Prepare test data.
		let account_id = H160::from_str("1000000000000000000000000000000000000001").unwrap();

		// Check test preconditions.
		assert!(!EvmSystem::account_exists(&account_id));

		// Set mock expectations.
		let on_new_account_ctx = MockDummyOnNewAccount::on_new_account_context();
		on_new_account_ctx
            .expect()
            .once()
            .with(
                predicate::eq(account_id),
            )
			.return_const(());

		// Invoke the function under test.
		assert_ok!(EvmSystem::create_account(&account_id));

		// Assert state changes.
        assert!(EvmSystem::account_exists(&account_id));

		// Assert mock invocations.
		on_new_account_ctx.checkpoint();
	});
}

#[test]
fn create_account_fails() {
    new_test_ext().execute_with(|| {
		// Prepare test data.
		let account_id = H160::from_str("1000000000000000000000000000000000000001").unwrap();
		<FullAccount<Test>>::insert(account_id.clone(), AccountInfo::<_, _>::default());

		// Invoke the function under test.
		assert_noop!(EvmSystem::create_account(&account_id), Error::<Test>::AccountAlreadyExist);
	});
}

#[test]
fn remove_account_works() {
    new_test_ext().execute_with(|| {
		// Prepare test data.
		let account_id = H160::from_str("1000000000000000000000000000000000000001").unwrap();
		<FullAccount<Test>>::insert(account_id.clone(), AccountInfo::<_, _>::default());

		// Set mock expectations.
		let on_killed_account_ctx = MockDummyOnKilledAccount::on_killed_account_context();
        on_killed_account_ctx
			.expect()
			.once()
			.with(
				predicate::eq(account_id),
			)
			.return_const(());

		// Invoke the function under test.
		assert_ok!(EvmSystem::remove_account(&account_id));

		// Assert mock invocations.
		on_killed_account_ctx.checkpoint();
	});
}

#[test]
fn remove_account_fails() {
    new_test_ext().execute_with(|| {
		// Prepare test data.
		let account_id = H160::from_str("1000000000000000000000000000000000000001").unwrap();

		// Invoke the function under test.
		assert_noop!(EvmSystem::remove_account(&account_id), Error::<Test>::AccountNotExist);
	});
}

#[test]
fn nonce_update_works() {
    new_test_ext().execute_with(|| {
		// Prepare test data.
		let account_id = H160::from_str("1000000000000000000000000000000000000001").unwrap();

		// Check test preconditions.
		let nonce_before = EvmSystem::account_nonce(&account_id);

		// Invoke the function under test.
		EvmSystem::inc_account_nonce(&account_id);

		// Assert state changes.
		assert_eq!(EvmSystem::account_nonce(&account_id), nonce_before + 1);
	});
}
