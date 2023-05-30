//! Unit tests.

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
