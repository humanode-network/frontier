//! Unit tests.

use sp_core::{H160, U256};
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
