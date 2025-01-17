//! Unit tests.

use sp_std::str::FromStr;

use frame_support::{assert_noop, assert_storage_noop};
use mockall::predicate;
use sp_core::H160;

use crate::{mock::*, *};

/// This test verifies that creating contract account works in the happy path.
#[test]
fn create_contract_account_works() {
	new_test_ext().execute_with_ext(|_| {
		// Prepare test data.
		let account_id = H160::from_str("1000000000000000000000000000000000000001").unwrap();

		// Check test preconditions.
		assert!(!EvmSystem::account_exists(&account_id));

		// Set block number to enable events.
		System::set_block_number(1);

		// Set mock expectations.
		let on_new_account_ctx = MockDummyOnNewAccount::on_new_account_context();
		on_new_account_ctx
			.expect()
			.once()
			.with(predicate::eq(account_id))
			.return_const(());

		// Invoke the function under test.
		assert_eq!(
			EvmSystem::create_contract_account(&account_id),
			AccountCreationOutcome::Created
		);

		// Assert state changes.
		assert!(EvmSystem::account_exists(&account_id));
		assert_eq!(
			<Account<Test>>::get(&account_id),
			AccountInfo {
				has_code: true,
				..Default::default()
			}
		);
		System::assert_has_event(RuntimeEvent::EvmSystem(Event::NewAccount {
			account: account_id,
		}));

		// Assert mock invocations.
		on_new_account_ctx.checkpoint();
	});
}

/// This test verifies that creating contract account fails when the account record already exists.
#[test]
fn create_contract_account_fails() {
	new_test_ext().execute_with_ext(|_| {
		// Prepare test data.
		let account_id = H160::from_str("1000000000000000000000000000000000000001").unwrap();
		let mut account_info = AccountInfo::<_, _>::default();
		account_info.has_code = true;
		<Account<Test>>::insert(account_id.clone(), account_info);

		// Invoke the function under test.
		assert_storage_noop!(assert_eq!(
			EvmSystem::create_contract_account(&account_id),
			AccountCreationOutcome::AlreadyExists
		));
	});
}

/// This test verifies that removing contract account works in the happy path.
#[test]
fn remove_contract_account_works() {
	new_test_ext().execute_with_ext(|_| {
		// Prepare test data.
		let account_id = H160::from_str("1000000000000000000000000000000000000001").unwrap();
		let mut account_info = AccountInfo::<_, _>::default();
		account_info.has_code = true;
		<Account<Test>>::insert(account_id.clone(), account_info);

		// Set block number to enable events.
		System::set_block_number(1);

		// Set mock expectations.
		let on_killed_account_ctx = MockDummyOnKilledAccount::on_killed_account_context();
		on_killed_account_ctx
			.expect()
			.once()
			.with(predicate::eq(account_id))
			.return_const(());

		// Invoke the function under test.
		assert_eq!(
			EvmSystem::remove_contract_account(&account_id),
			AccountRemovalOutcome::Reaped
		);

		// Assert state changes.
		assert!(!EvmSystem::account_exists(&account_id));
		System::assert_has_event(RuntimeEvent::EvmSystem(Event::KilledAccount {
			account: account_id,
		}));

		// Assert mock invocations.
		on_killed_account_ctx.checkpoint();
	});
}

/// This test verifies that removing contract account fails when the account doesn't exist.
#[test]
fn remove_contract_account_code_fails_did_not_exist() {
	new_test_ext().execute_with_ext(|_| {
		// Prepare test data.
		let account_id = H160::from_str("1000000000000000000000000000000000000001").unwrap();

		// Invoke the function under test.
		assert_storage_noop!(assert_eq!(
			EvmSystem::remove_contract_account(&account_id),
			AccountRemovalOutcome::DidNotExist
		));
	});
}

/// This test verifies that removing contract account fails when the account record
/// indicator about code existence is false.
#[test]
fn remove_contract_account_code_fails_has_code_false() {
	new_test_ext().execute_with_ext(|_| {
		// Prepare test data.
		let account_id = H160::from_str("1000000000000000000000000000000000000001").unwrap();
		let mut account_info = AccountInfo::<_, _>::default();
		account_info.has_code = false;
		<Account<Test>>::insert(account_id.clone(), account_info);

		// Invoke the function under test.
		assert_storage_noop!(assert_eq!(
			EvmSystem::remove_contract_account(&account_id),
			AccountRemovalOutcome::Retained
		));
	});
}

/// This test verifies that removing contract account fails when the account record
/// contains some account data.
#[test]
fn remove_contract_account_code_fails_some_account_data() {
	new_test_ext().execute_with_ext(|_| {
		// Prepare test data.
		let account_id = H160::from_str("1000000000000000000000000000000000000001").unwrap();
		let mut account_info = AccountInfo::<_, _>::default();
		account_info.data = 10;
		<Account<Test>>::insert(account_id.clone(), account_info);

		// Invoke the function under test.
		assert_storage_noop!(assert_eq!(
			EvmSystem::remove_contract_account(&account_id),
			AccountRemovalOutcome::Retained
		));
	});
}

/// This test verifies that incrementing account nonce works in the happy path.
#[test]
fn inc_account_nonce_works() {
	new_test_ext().execute_with_ext(|_| {
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

/// This test verifies that try_mutate_exists works as expected in case data wasn't providing
/// and returned data is `Some`. As a result, a new account has been created.
#[test]
fn try_mutate_exists_account_created() {
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
			.with(predicate::eq(account_id))
			.return_const(());

		// Set block number to enable events.
		System::set_block_number(1);

		// Invoke the function under test.
		EvmSystem::try_mutate_exists(&account_id, |maybe_data| -> Result<(), DispatchError> {
			*maybe_data = Some(1);
			Ok(())
		})
		.unwrap();

		// Assert state changes.
		assert!(EvmSystem::account_exists(&account_id));
		assert_eq!(
			<Account<Test>>::get(&account_id),
			AccountInfo {
				data: 1,
				..Default::default()
			}
		);
		System::assert_has_event(RuntimeEvent::EvmSystem(Event::NewAccount {
			account: account_id,
		}));

		// Assert mock invocations.
		on_new_account_ctx.checkpoint();
	});
}

/// This test verifies that try_mutate_exists works as expected in case data was providing
/// and returned data is `Some`. As a result, the account has been updated.
#[test]
fn try_mutate_exists_account_updated() {
	new_test_ext().execute_with_ext(|_| {
		// Prepare test data.
		let account_id = H160::from_str("1000000000000000000000000000000000000001").unwrap();
		<Account<Test>>::insert(account_id.clone(), AccountInfo::<_, _>::default());

		// Check test preconditions.
		assert!(EvmSystem::account_exists(&account_id));

		// Set block number to enable events.
		System::set_block_number(1);

		// Invoke the function under test.
		EvmSystem::try_mutate_exists(&account_id, |maybe_data| -> Result<(), DispatchError> {
			if let Some(ref mut data) = maybe_data {
				*data += 1;
			}
			Ok(())
		})
		.unwrap();

		// Assert state changes.
		assert!(EvmSystem::account_exists(&account_id));
		assert_eq!(
			<Account<Test>>::get(&account_id),
			AccountInfo {
				data: 1,
				..Default::default()
			}
		);
	});
}

/// This test verifies that try_mutate_exists works as expected in case data was providing
/// and returned data is `None`, account doesn't have code. As a result, the account has been removed.
#[test]
fn try_mutate_exists_not_contract_account_removed() {
	new_test_ext().execute_with_ext(|_| {
		// Prepare test data.
		let account_id = H160::from_str("1000000000000000000000000000000000000001").unwrap();
		<Account<Test>>::insert(account_id.clone(), AccountInfo::<_, _>::default());

		// Check test preconditions.
		assert!(EvmSystem::account_exists(&account_id));

		// Set mock expectations.
		let on_killed_account_ctx = MockDummyOnKilledAccount::on_killed_account_context();
		on_killed_account_ctx
			.expect()
			.once()
			.with(predicate::eq(account_id))
			.return_const(());

		// Set block number to enable events.
		System::set_block_number(1);

		// Invoke the function under test.
		EvmSystem::try_mutate_exists(&account_id, |maybe_data| -> Result<(), DispatchError> {
			*maybe_data = None;
			Ok(())
		})
		.unwrap();

		// Assert state changes.
		assert!(!EvmSystem::account_exists(&account_id));
		System::assert_has_event(RuntimeEvent::EvmSystem(Event::KilledAccount {
			account: account_id,
		}));

		// Assert mock invocations.
		on_killed_account_ctx.checkpoint();
	});
}

/// This test verifies that try_mutate_exists works as expected in case data was providing
/// and returned data is `None`, account has code. As a result, the account has been retained.
#[test]
fn try_mutate_exists_contract_account_retained() {
	new_test_ext().execute_with_ext(|_| {
		// Prepare test data.
		let account_id = H160::from_str("1000000000000000000000000000000000000001").unwrap();
		let mut account_info = AccountInfo::<_, _>::default();
		account_info.has_code = true;
		<Account<Test>>::insert(account_id.clone(), account_info);

		// Check test preconditions.
		assert!(EvmSystem::account_exists(&account_id));

		// Invoke the function under test.
		EvmSystem::try_mutate_exists(&account_id, |maybe_data| -> Result<(), DispatchError> {
			*maybe_data = None;
			Ok(())
		})
		.unwrap();

		// Assert state changes.
		assert!(EvmSystem::account_exists(&account_id));
		assert_eq!(
			<Account<Test>>::get(&account_id),
			AccountInfo {
				has_code: true,
				..Default::default()
			}
		);
	});
}

/// This test verifies that try_mutate_exists works as expected in case data wasn't providing
/// and returned data is `None`. As a result, the account hasn't been created.
#[test]
fn try_mutate_exists_account_not_created() {
	new_test_ext().execute_with_ext(|_| {
		// Prepare test data.
		let account_id = H160::from_str("1000000000000000000000000000000000000001").unwrap();

		// Check test preconditions.
		assert!(!EvmSystem::account_exists(&account_id));

		// Set block number to enable events.
		System::set_block_number(1);

		// Invoke the function under test.
		<Account<Test>>::try_mutate_exists(account_id, |maybe_data| -> Result<(), ()> {
			*maybe_data = None;
			Ok(())
		})
		.unwrap();

		// Assert state changes.
		assert!(!EvmSystem::account_exists(&account_id));
	});
}

/// This test verifies that try_mutate_exists works as expected in case getting error
/// during data mutation.
#[test]
fn try_mutate_exists_fails_without_changes() {
	new_test_ext().execute_with_ext(|_| {
		// Prepare test data.
		let account_id = H160::from_str("1000000000000000000000000000000000000001").unwrap();
		<Account<Test>>::insert(account_id.clone(), AccountInfo::<_, _>::default());

		// Check test preconditions.
		assert!(EvmSystem::account_exists(&account_id));

		// Invoke the function under test.
		assert_noop!(
			<Account<Test>>::try_mutate_exists(account_id, |maybe_data| -> Result<(), ()> {
				*maybe_data = None;
				Err(())
			}),
			()
		);

		// Assert state changes.
		assert!(EvmSystem::account_exists(&account_id));
		assert_eq!(
			<Account<Test>>::get(&account_id),
			AccountInfo::<_, _>::default()
		);
	});
}
