//! Tests regarding the functionality of the fungible trait set implementations.

use frame_support::{
	assert_noop, assert_ok,
	traits::{
		fungible::{Inspect, Mutate, Unbalanced},
		tokens::Precision,
	},
};
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

#[test]
fn write_balance_works() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);

		let write_balance = 10;

		// Invoke the function under test.
		assert_eq!(
			EvmBalances::write_balance(&alice(), write_balance),
			Ok(None)
		);

		// Assert state changes.
		assert_eq!(EvmBalances::total_balance(&alice()), write_balance);
	});
}

#[test]
fn set_total_issuance_works() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_issuance(), 2 * INIT_BALANCE);

		let set_total_issuance_balance = 100;

		// Invoke the function under test.
		EvmBalances::set_total_issuance(set_total_issuance_balance);

		// Assert state changes.
		assert_eq!(EvmBalances::total_issuance(), set_total_issuance_balance);
	});
}

#[test]
fn decrease_balance_works_exact_expendable() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);

		let decreased_balance = 100;

		// Invoke the function under test.
		assert_ok!(EvmBalances::decrease_balance(
			&alice(),
			decreased_balance,
			Precision::Exact,
			Preservation::Expendable,
			Fortitude::Polite
		));

		// Assert state changes.
		assert_eq!(
			EvmBalances::total_balance(&alice()),
			INIT_BALANCE - decreased_balance
		);
	});
}

#[test]
fn decrease_balance_works_best_effort_preserve() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);

		let decreased_balance = INIT_BALANCE + 1;

		// Invoke the function under test.
		assert_ok!(EvmBalances::decrease_balance(
			&alice(),
			decreased_balance,
			Precision::BestEffort,
			Preservation::Preserve,
			Fortitude::Polite
		));

		// Assert state changes.
		assert_eq!(EvmBalances::total_balance(&alice()), 1);
	});
}

#[test]
fn decrease_balance_works_full_balance() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);

		// Set block number to enable events.
		System::set_block_number(1);

		let decreased_balance = INIT_BALANCE;

		// Invoke the function under test.
		assert_ok!(EvmBalances::decrease_balance(
			&alice(),
			decreased_balance,
			Precision::Exact,
			Preservation::Expendable,
			Fortitude::Polite
		));

		// Assert state changes.
		assert_eq!(EvmBalances::total_balance(&alice()), 0);
		assert!(!EvmSystem::account_exists(&alice()));
		System::assert_has_event(RuntimeEvent::EvmSystem(
			pallet_evm_system::Event::KilledAccount { account: alice() },
		));
	});
}

#[test]
fn decrease_balance_fails_funds_unavailable() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);

		let decreased_balance = INIT_BALANCE + 1;

		// Invoke the function under test.
		assert_noop!(
			EvmBalances::decrease_balance(
				&alice(),
				decreased_balance,
				Precision::Exact,
				Preservation::Preserve,
				Fortitude::Polite
			),
			TokenError::FundsUnavailable
		);
	});
}

#[test]
fn increase_balance_works() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);

		let increased_balance = 100;

		// Invoke the function under test.
		assert_ok!(EvmBalances::increase_balance(
			&alice(),
			increased_balance,
			Precision::Exact,
		));

		// Assert state changes.
		assert_eq!(
			EvmBalances::total_balance(&alice()),
			INIT_BALANCE + increased_balance
		);
	});
}

#[test]
fn increase_balance_works_best_effort() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);

		let increased_balance = u64::MAX;

		// Invoke the function under test.
		assert_ok!(EvmBalances::increase_balance(
			&alice(),
			increased_balance,
			Precision::BestEffort,
		));

		// Assert state changes.
		assert_eq!(EvmBalances::total_balance(&alice()), u64::MAX);
	});
}

#[test]
fn increase_balance_fails_overflow() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);

		let increased_balance = u64::MAX;

		// Invoke the function under test.
		assert_noop!(
			EvmBalances::increase_balance(&alice(), increased_balance, Precision::Exact),
			ArithmeticError::Overflow
		);
	});
}

#[test]
fn deactivate_reactivate_works() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(<InactiveIssuance<Test>>::get(), 0);

		// Deactivate some balance.
		EvmBalances::deactivate(100);
		// Assert state changes.
		assert_eq!(<InactiveIssuance<Test>>::get(), 100);

		// Reactivate some balance.
		EvmBalances::reactivate(40);
		// Assert state changes.
		assert_eq!(<InactiveIssuance<Test>>::get(), 60);
	});
}

#[test]
fn mint_into_works() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);
		assert_eq!(EvmBalances::total_issuance(), 2 * INIT_BALANCE);

		// Set block number to enable events.
		System::set_block_number(1);

		let minted_balance = 10;

		// Invoke the function under test.
		assert_ok!(EvmBalances::mint_into(&alice(), minted_balance));

		// Assert state changes.
		assert_eq!(
			EvmBalances::total_balance(&alice()),
			INIT_BALANCE + minted_balance
		);
		assert_eq!(
			EvmBalances::total_issuance(),
			2 * INIT_BALANCE + minted_balance
		);
		System::assert_has_event(RuntimeEvent::EvmBalances(Event::Minted {
			who: alice(),
			amount: minted_balance,
		}));
	});
}

#[test]
fn mint_into_fails_overflow() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);

		let minted_balance = u64::MAX;

		// Invoke the function under test.
		assert_noop!(
			EvmBalances::mint_into(&alice(), minted_balance),
			ArithmeticError::Overflow
		);
	});
}

#[test]
fn burn_from_works() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);
		assert_eq!(EvmBalances::total_issuance(), 2 * INIT_BALANCE);

		// Set block number to enable events.
		System::set_block_number(1);

		let burned_balance = 10;

		// Invoke the function under test.
		assert_ok!(EvmBalances::burn_from(
			&alice(),
			burned_balance,
			Precision::Exact,
			Fortitude::Polite
		));

		// Assert state changes.
		assert_eq!(
			EvmBalances::total_balance(&alice()),
			INIT_BALANCE - burned_balance
		);
		assert_eq!(
			EvmBalances::total_issuance(),
			2 * INIT_BALANCE - burned_balance
		);
		System::assert_has_event(RuntimeEvent::EvmBalances(Event::Burned {
			who: alice(),
			amount: burned_balance,
		}));
	});
}

#[test]
fn burn_from_works_best_effort() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);
		assert_eq!(EvmBalances::total_issuance(), 2 * INIT_BALANCE);

		// Set block number to enable events.
		System::set_block_number(1);

		let burned_balance = INIT_BALANCE + 1;

		// Invoke the function under test.
		assert_ok!(EvmBalances::burn_from(
			&alice(),
			burned_balance,
			Precision::BestEffort,
			Fortitude::Polite
		));

		// Assert state changes.
		assert_eq!(EvmBalances::total_balance(&alice()), 0);
		assert_eq!(EvmBalances::total_issuance(), INIT_BALANCE);
		System::assert_has_event(RuntimeEvent::EvmBalances(Event::Burned {
			who: alice(),
			amount: INIT_BALANCE,
		}));
		assert!(!EvmSystem::account_exists(&alice()));
		System::assert_has_event(RuntimeEvent::EvmSystem(
			pallet_evm_system::Event::KilledAccount { account: alice() },
		));
	});
}

#[test]
fn burn_from_works_exact_full_balance() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);
		assert_eq!(EvmBalances::total_issuance(), 2 * INIT_BALANCE);

		// Set block number to enable events.
		System::set_block_number(1);

		let burned_balance = INIT_BALANCE;

		// Invoke the function under test.
		assert_ok!(EvmBalances::burn_from(
			&alice(),
			burned_balance,
			Precision::Exact,
			Fortitude::Polite
		));

		// Assert state changes.
		assert_eq!(EvmBalances::total_balance(&alice()), 0);
		assert_eq!(EvmBalances::total_issuance(), INIT_BALANCE);
		System::assert_has_event(RuntimeEvent::EvmBalances(Event::Burned {
			who: alice(),
			amount: INIT_BALANCE,
		}));
		assert!(!EvmSystem::account_exists(&alice()));
		System::assert_has_event(RuntimeEvent::EvmSystem(
			pallet_evm_system::Event::KilledAccount { account: alice() },
		));
	});
}

#[test]
fn burn_from_fails_funds_unavailable() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);
		assert_eq!(EvmBalances::total_issuance(), 2 * INIT_BALANCE);

		// Set block number to enable events.
		System::set_block_number(1);

		let burned_balance = INIT_BALANCE + 1;

		// Invoke the function under test.
		assert_noop!(
			EvmBalances::burn_from(
				&alice(),
				burned_balance,
				Precision::Exact,
				Fortitude::Polite
			),
			TokenError::FundsUnavailable
		);
	});
}

#[test]
fn shelve_works() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);
		assert_eq!(EvmBalances::total_issuance(), 2 * INIT_BALANCE);

		// Set block number to enable events.
		System::set_block_number(1);

		let shelved_balance = 10;

		// Invoke the function under test.
		assert_ok!(EvmBalances::shelve(&alice(), shelved_balance));

		// Assert state changes.
		assert_eq!(
			EvmBalances::total_balance(&alice()),
			INIT_BALANCE - shelved_balance
		);
		assert_eq!(
			EvmBalances::total_issuance(),
			2 * INIT_BALANCE - shelved_balance
		);
		System::assert_has_event(RuntimeEvent::EvmBalances(Event::Suspended {
			who: alice(),
			amount: shelved_balance,
		}));
	});
}

#[test]
fn shelve_works_exact_full_balance() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);
		assert_eq!(EvmBalances::total_issuance(), 2 * INIT_BALANCE);

		// Set block number to enable events.
		System::set_block_number(1);

		let shelved_balance = INIT_BALANCE;

		// Invoke the function under test.
		assert_ok!(EvmBalances::shelve(&alice(), shelved_balance));

		// Assert state changes.
		assert_eq!(EvmBalances::total_balance(&alice()), 0);
		assert_eq!(EvmBalances::total_issuance(), INIT_BALANCE);
		System::assert_has_event(RuntimeEvent::EvmBalances(Event::Suspended {
			who: alice(),
			amount: INIT_BALANCE,
		}));
		assert!(!EvmSystem::account_exists(&alice()));
		System::assert_has_event(RuntimeEvent::EvmSystem(
			pallet_evm_system::Event::KilledAccount { account: alice() },
		));
	});
}

#[test]
fn shelve_fails_funds_unavailable() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);
		assert_eq!(EvmBalances::total_issuance(), 2 * INIT_BALANCE);

		// Set block number to enable events.
		System::set_block_number(1);

		let shelved_balance = INIT_BALANCE + 1;

		// Invoke the function under test.
		assert_noop!(
			EvmBalances::shelve(&alice(), shelved_balance),
			TokenError::FundsUnavailable
		);
	});
}

#[test]
fn restore_works() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);
		assert_eq!(EvmBalances::total_issuance(), 2 * INIT_BALANCE);

		// Set block number to enable events.
		System::set_block_number(1);

		let restored_balance = 10;

		// Invoke the function under test.
		assert_ok!(EvmBalances::restore(&alice(), restored_balance));

		// Assert state changes.
		assert_eq!(
			EvmBalances::total_balance(&alice()),
			INIT_BALANCE + restored_balance
		);
		assert_eq!(
			EvmBalances::total_issuance(),
			2 * INIT_BALANCE + restored_balance
		);
		System::assert_has_event(RuntimeEvent::EvmBalances(Event::Restored {
			who: alice(),
			amount: restored_balance,
		}));
	});
}

#[test]
fn restore_fails_overflow() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);

		let restored_balance = u64::MAX;

		// Invoke the function under test.
		assert_noop!(
			EvmBalances::restore(&alice(), restored_balance),
			ArithmeticError::Overflow
		);
	});
}

#[test]
fn transfer_works() {
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
			Preservation::Preserve
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
		System::assert_has_event(RuntimeEvent::EvmBalances(Event::Transfer {
			from: alice(),
			to: bob(),
			amount: transfered_amount,
		}));
	});
}

#[test]
fn transfer_works_full_balance() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);

		let transfered_amount = INIT_BALANCE;

		// Set block number to enable events.
		System::set_block_number(1);

		// Invoke the function under test.
		assert_ok!(EvmBalances::transfer(
			&alice(),
			&bob(),
			transfered_amount,
			Preservation::Expendable
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
		System::assert_has_event(RuntimeEvent::EvmBalances(Event::Transfer {
			from: alice(),
			to: bob(),
			amount: transfered_amount,
		}));
		assert!(!EvmSystem::account_exists(&alice()));
		System::assert_has_event(RuntimeEvent::EvmSystem(
			pallet_evm_system::Event::KilledAccount { account: alice() },
		));
	});
}

#[test]
fn transfer_fails_funds_unavailable() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);

		let transfered_amount = INIT_BALANCE + 1;

		// Set block number to enable events.
		System::set_block_number(1);

		// Invoke the function under test.
		assert_noop!(
			EvmBalances::transfer(&alice(), &bob(), transfered_amount, Preservation::Preserve),
			TokenError::FundsUnavailable
		);
	});
}

#[test]
fn transfer_fails_not_expendable() {
	new_test_ext().execute_with_ext(|_| {
		// Check test preconditions.
		assert_eq!(EvmBalances::total_balance(&alice()), INIT_BALANCE);

		let transfered_amount = INIT_BALANCE;

		// Set block number to enable events.
		System::set_block_number(1);

		// Invoke the function under test.
		assert_noop!(
			EvmBalances::transfer(&alice(), &bob(), transfered_amount, Preservation::Preserve),
			TokenError::NotExpendable
		);
	});
}

#[test]
fn transfer_fails_underflow() {
	new_test_ext().execute_with(|| {
		// Prepare test preconditions.
		let charlie = H160::from_str("1000000000000000000000000000000000000003").unwrap();
		let eve = H160::from_str("1000000000000000000000000000000000000004").unwrap();
		EvmBalances::set_balance(&charlie, u64::MAX);
		EvmBalances::set_balance(&eve, 1);

		// Invoke the function under test.
		assert_noop!(
			EvmBalances::transfer(&charlie, &eve, u64::MAX, Preservation::Expendable),
			// Withdraw consequence is checked first by reducing total issuance.
			ArithmeticError::Underflow,
		);
	});
}
