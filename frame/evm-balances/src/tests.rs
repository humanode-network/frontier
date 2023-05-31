//! Unit tests.

use frame_support::assert_ok;
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
			<EvmSystem>::full_account(&alice()),
			pallet_evm_system::AccountInfo {
				nonce: 0,
				data: account_data::AccountData { free: INIT_BALANCE }
			}
		);
		assert_eq!(
			<EvmSystem>::full_account(&bob()),
			pallet_evm_system::AccountInfo {
				nonce: 0,
				data: account_data::AccountData { free: INIT_BALANCE }
			}
		);

		// Check the total balance value.
		assert_eq!(EvmBalances::total_issuance(), 2 * INIT_BALANCE);
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

		// Set block number to enable events.
		System::set_block_number(1);

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
