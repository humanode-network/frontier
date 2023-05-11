//! Unit tests.

use core::str::FromStr;

use frame_support::{assert_ok, assert_noop};
use mockall::predicate;
use sp_core::H160;

use crate::{mock::*, *};

#[test]
fn create_account_works() {
    new_test_ext().execute_with_ext(|_| {
		let account_id = H160::from_str("1000000000000000000000000000000000000001").unwrap();

		let on_new_account_ctx = MockDummyOnNewAccount::on_new_account_context();
		on_new_account_ctx
            .expect()
            .once()
            .with(
                predicate::eq(account_id),
            )
			.return_const(());

		assert_ok!(EvmSystem::create_account(&account_id));

		on_new_account_ctx.checkpoint();
	});
}

#[test]
fn create_account_fails() {
    new_test_ext().execute_with(|| {
		let account_id = H160::from_str("1000000000000000000000000000000000000001").unwrap();
		<FullAccount<Test>>::insert(account_id.clone(), AccountInfo::<_, _>::default());

		let on_new_account_ctx = MockDummyOnNewAccount::on_new_account_context();
		on_new_account_ctx.expect().never();

		assert_noop!(EvmSystem::create_account(&account_id), Error::<Test>::AccountAlreadyExist);

		on_new_account_ctx.checkpoint();
	});
}

#[test]
fn remove_account_works() {
    new_test_ext().execute_with(|| {
		let account_id = H160::from_str("1000000000000000000000000000000000000001").unwrap();
		<FullAccount<Test>>::insert(account_id.clone(), AccountInfo::<_, _>::default());

		let on_killed_account_ctx = MockDummyOnKilledAccount::on_killed_account_context();
        on_killed_account_ctx
			.expect()
			.once()
			.with(
				predicate::eq(account_id),
			)
			.return_const(());

		assert_ok!(EvmSystem::remove_account(&account_id));

		on_killed_account_ctx.checkpoint();
	});
}

#[test]
fn remove_account_fails() {
    new_test_ext().execute_with(|| {
		let account_id = H160::from_str("1000000000000000000000000000000000000001").unwrap();

		let on_killed_account_ctx = MockDummyOnKilledAccount::on_killed_account_context();
        on_killed_account_ctx.expect().never();

		assert_noop!(EvmSystem::remove_account(&account_id), Error::<Test>::AccountNotExist);

		on_killed_account_ctx.checkpoint();
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
