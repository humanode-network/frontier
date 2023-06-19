//! Unit tests.

use frame_support::{assert_ok, weights::Weight};
use pallet_evm::{FeeCalculator, Runner};
use sp_core::{H160, U256};
use sp_runtime::traits::UniqueSaturatedInto;
use sp_std::str::FromStr;

use crate::{mock::*, *};

#[test]
fn basic_setup_works() {
	new_test_ext().execute_with_ext(|_| {
		// Check the accounts.
		assert_eq!(
			<EvmBalances>::account(&alice()),
			account_data::AccountData { free: INIT_BALANCE }
		);
		assert_eq!(
			<EvmBalances>::account(&bob()),
			account_data::AccountData { free: INIT_BALANCE }
		);

		// Check the total balance value.
		assert_eq!(EvmBalances::total_issuance(), 2 * INIT_BALANCE);
	});
}

#[test]
fn balance_transfer_works() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);

		let transfered_amount = 100;

		// Set block number to enable events.
		System::set_block_number(1);

		// Invoke the function under test.
		assert_ok!(EvmBalances::transfer(
			&alice(),
			&bob(),
			transfered_amount,
			ExistenceRequirement::KeepAlive
		));

		// Assert state changes.
		assert_eq!(
			EvmBalances::total_balance(&alice()),
			INIT_BALANCE - transfered_amount
		);
		assert_eq!(
			EvmBalances::total_balance(&bob()),
			INIT_BALANCE + transfered_amount
		);
		System::assert_has_event(RuntimeEvent::EvmBalances(crate::Event::Transfer {
			from: alice(),
			to: bob(),
			amount: transfered_amount,
		}));
	});
}

#[test]
fn deposit_into_existing_works() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);

		let deposited_amount = 10;

		// Set block number to enable events.
		System::set_block_number(1);

		// Invoke the function under test.
		assert_ok!(EvmBalances::deposit_into_existing(
			&alice(),
			deposited_amount
		));

		// Assert state changes.
		assert_eq!(
			EvmBalances::total_balance(&alice()),
			INIT_BALANCE + deposited_amount
		);
		System::assert_has_event(RuntimeEvent::EvmBalances(crate::Event::Deposit {
			who: alice(),
			amount: deposited_amount,
		}));
	});
}

#[test]
fn slashing_balance_works() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);

		let slashed_amount = 1000;

		// Set block number to enable events.
		System::set_block_number(1);

		// Invoke the function under test.
		assert!(EvmBalances::slash(&alice(), 1000).1.is_zero());

		// Assert state changes.
		assert_eq!(
			EvmBalances::total_balance(&alice()),
			INIT_BALANCE - slashed_amount
		);
		System::assert_has_event(RuntimeEvent::EvmBalances(crate::Event::Slashed {
			who: alice(),
			amount: slashed_amount,
		}));
	});
}

#[test]
fn withdraw_balance_works() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);

		let withdrawed_amount = 1000;

		// Set block number to enable events.
		System::set_block_number(1);

		// Invoke the function under test.
		assert_ok!(EvmBalances::withdraw(
			&alice(),
			1000,
			WithdrawReasons::FEE,
			ExistenceRequirement::KeepAlive
		));

		// Assert state changes.
		assert_eq!(
			EvmBalances::total_balance(&alice()),
			INIT_BALANCE - withdrawed_amount
		);
		System::assert_has_event(RuntimeEvent::EvmBalances(crate::Event::Withdraw {
			who: alice(),
			amount: withdrawed_amount,
		}));
	});
}

#[test]
fn account_should_be_reaped() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert!(EvmSystem::account_exists(&bob()));

		// Invoke the function under test.
		assert_ok!(EvmBalances::transfer(
			&bob(),
			&alice(),
			INIT_BALANCE,
			ExistenceRequirement::AllowDeath
		));

		// Assert state changes.
		assert_eq!(EvmBalances::free_balance(&bob()), 0);
		assert!(!EvmSystem::account_exists(&bob()));
	});
}

#[test]
fn fee_deduction() {
	new_test_ext().execute_with(|| {
		let charlie = H160::from_str("1000000000000000000000000000000000000003").unwrap();

		// Seed account
		let _ = <Test as pallet_evm::Config>::Currency::deposit_creating(&charlie, 100);
		assert_eq!(EvmBalances::free_balance(&charlie), 100);

		// Deduct fees as 10 units
		let imbalance =
			<<Test as pallet_evm::Config>::OnChargeTransaction as pallet_evm::OnChargeEVMTransaction<Test>>::withdraw_fee(
				&charlie,
				U256::from(10),
			)
			.unwrap();
		assert_eq!(EvmBalances::free_balance(&charlie), 90);

		// Refund fees as 5 units
		<<Test as pallet_evm::Config>::OnChargeTransaction as pallet_evm::OnChargeEVMTransaction<Test>>::correct_and_deposit_fee(&charlie, U256::from(5), U256::from(5), imbalance);
		assert_eq!(EvmBalances::free_balance(&charlie), 95);
	});
}

#[test]
fn issuance_after_tip() {
	new_test_ext().execute_with(|| {
		let before_tip = <Test as pallet_evm::Config>::Currency::total_issuance();

		assert_ok!(<Test as pallet_evm::Config>::Runner::call(
			alice(),
			bob(),
			Vec::new(),
			U256::from(1),
			1000000,
			Some(U256::from(2_000_000_000)),
			None,
			None,
			Vec::new(),
			true,
			true,
			<Test as pallet_evm::Config>::config(),
		));

		// Only base fee is burned
		let base_fee: u64 = <Test as pallet_evm::Config>::FeeCalculator::min_gas_price()
			.0
			.unique_saturated_into();

		let after_tip = <Test as pallet_evm::Config>::Currency::total_issuance();

		assert_eq!(after_tip, (before_tip - (base_fee * 21_000)));
	});
}

#[test]
fn refunds_should_work() {
	new_test_ext().execute_with(|| {
		let before_call = EVM::account_basic(&alice()).0.balance;
		// Gas price is not part of the actual fee calculations anymore, only the base fee.
		//
		// Because we first deduct max_fee_per_gas * gas_limit (2_000_000_000 * 1000000) we need
		// to ensure that the difference (max fee VS base fee) is refunded.

		let _ = <Test as pallet_evm::Config>::Runner::call(
			alice(),
			bob(),
			Vec::new(),
			U256::from(1),
			1000000,
			Some(U256::from(2_000_000_000)),
			None,
			None,
			Vec::new(),
			true,
			true,
			<Test as pallet_evm::Config>::config(),
		);

		let (base_fee, _) = <Test as pallet_evm::Config>::FeeCalculator::min_gas_price();
		let total_cost = (U256::from(21_000) * base_fee) + U256::from(1);
		let after_call = EVM::account_basic(&alice()).0.balance;
		assert_eq!(after_call, before_call - total_cost);
	});
}

#[test]
fn refunds_and_priority_should_work() {
	new_test_ext().execute_with(|| {
		let before_call = EVM::account_basic(&alice()).0.balance;
		// We deliberately set a base fee + max tip > max fee.
		// The effective priority tip will be 1GWEI instead 1.5GWEI:
		// 		(max_fee_per_gas - base_fee).min(max_priority_fee)
		//		(2 - 1).min(1.5)
		let tip = U256::from(1_500_000_000);
		let max_fee_per_gas = U256::from(2_000_000_000);
		let used_gas = U256::from(21_000);

		let _ = <Test as pallet_evm::Config>::Runner::call(
			alice(),
			bob(),
			Vec::new(),
			U256::from(1),
			1000000,
			Some(max_fee_per_gas),
			Some(tip),
			None,
			Vec::new(),
			true,
			true,
			<Test as pallet_evm::Config>::config(),
		);

		let (base_fee, _) = <Test as pallet_evm::Config>::FeeCalculator::min_gas_price();
		let actual_tip = (max_fee_per_gas - base_fee).min(tip) * used_gas;
		let total_cost = (used_gas * base_fee) + U256::from(actual_tip) + U256::from(1);
		let after_call = EVM::account_basic(&alice()).0.balance;
		// The tip is deducted but never refunded to the caller.
		assert_eq!(after_call, before_call - total_cost);
	});
}

#[test]
fn call_should_fail_with_priority_greater_than_max_fee() {
	new_test_ext().execute_with(|| {
		// Max priority greater than max fee should fail.
		let tip: u128 = 1_100_000_000;

		let result = <Test as pallet_evm::Config>::Runner::call(
			alice(),
			bob(),
			Vec::new(),
			U256::from(1),
			1000000,
			Some(U256::from(1_000_000_000)),
			Some(U256::from(tip)),
			None,
			Vec::new(),
			true,
			true,
			<Test as pallet_evm::Config>::config(),
		);
		assert!(result.is_err());
		// Some used weight is returned as part of the error.
		assert_eq!(result.unwrap_err().weight, Weight::from_ref_time(7));
	});
}

#[test]
fn call_should_succeed_with_priority_equal_to_max_fee() {
	new_test_ext().execute_with(|| {
		let tip: u128 = 1_000_000_000;
		// Mimics the input for pre-eip-1559 transaction types where `gas_price`
		// is used for both `max_fee_per_gas` and `max_priority_fee_per_gas`.
		let result = <Test as pallet_evm::Config>::Runner::call(
			alice(),
			bob(),
			Vec::new(),
			U256::from(1),
			1000000,
			Some(U256::from(1_000_000_000)),
			Some(U256::from(tip)),
			None,
			Vec::new(),
			true,
			true,
			<Test as pallet_evm::Config>::config(),
		);
		assert!(result.is_ok());
	});
}
