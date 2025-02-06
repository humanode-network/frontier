//! Migration to recover broken nonces.

#[cfg(feature = "try-runtime")]
use frame_support::sp_std::vec::Vec;
use frame_support::{log::info, pallet_prelude::*, traits::OnRuntimeUpgrade};
use rlp::RlpStream;
use sp_core::H160;
use sp_io::hashing::keccak_256;
use sp_runtime::traits::Zero;

use crate::{Account, AccountInfo, Config, Pallet};

/// EVM provider interface.
pub trait EvmProvider<AccountId> {
	/// Check whether account is managed by EVM or not.
	fn is_managed_by_evm(account_id: &AccountId) -> (Weight, bool);
}

/// Execute migration to recover broken nonces.
pub struct MigrationBrokenNoncesRecover<EP, T>(sp_std::marker::PhantomData<(EP, T)>);

#[cfg(feature = "try-runtime")]
#[derive(Encode, Decode)]
struct PreUpgradeState {
	accounts: u64,
}

impl<EP, T> OnRuntimeUpgrade for MigrationBrokenNoncesRecover<EP, T>
where
	EP: EvmProvider<<T as Config>::AccountId>,
	T: Config<AccountId = H160>,
	<T as Config>::Index: rlp::Encodable,
{
	fn on_runtime_upgrade() -> Weight {
		let pallet_name = Pallet::<T>::name();

		info!(
			"{}: Running migration to recover broken nonces",
			pallet_name
		);

		let mut weight: Weight = T::DbWeight::get().reads(1);

		<Account<T>>::translate::<AccountInfo<<T as Config>::Index, <T as Config>::AccountData>, _>(
			|account_id, old_account| {
				let (w, account_info) = Self::recover(account_id, old_account);
				weight.saturating_accrue(w);
				weight.saturating_accrue(T::DbWeight::get().reads_writes(1, 1));
				Some(account_info)
			},
		);

		info!("{}: Migrated", pallet_name);

		weight
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
		let accounts = <Account<T>>::iter_keys()
			.count()
			.try_into()
			.expect("Accounts count must not overflow");
		Ok(PreUpgradeState { accounts }.encode())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(state: Vec<u8>) -> Result<(), &'static str> {
		let accounts: u64 = <Account<T>>::iter_keys()
			.count()
			.try_into()
			.expect("Accounts count must not overflow");
		let PreUpgradeState {
			accounts: expected_accounts_count,
		} = Decode::decode(&mut state.as_slice())
			.map_err(|_err| "Failed pre-upgrade state decoding")?;
		ensure!(
			accounts == expected_accounts_count,
			"Accounts count shouldn't change",
		);

		let accounts_to_recover = <Account<T>>::iter()
			.filter(|(account_id, account)| {
				let (_weight, is_broken) = Self::has_broken_nonce(&account_id, &account);
				is_broken
			})
			.count();
		ensure!(
			accounts_to_recover == 0,
			"There should be no accounts left for recovery",
		);
		Ok(())
	}
}

impl<EP, T> MigrationBrokenNoncesRecover<EP, T>
where
	EP: EvmProvider<<T as Config>::AccountId>,
	T: Config<AccountId = H160>,
	<T as Config>::Index: rlp::Encodable,
{
	fn recover(
		account_id: <T as Config>::AccountId,
		old_account: AccountInfo<<T as Config>::Index, <T as Config>::AccountData>,
	) -> (
		Weight,
		AccountInfo<<T as Config>::Index, <T as Config>::AccountData>,
	) {
		let (mut weight, is_broken) = Self::has_broken_nonce(&account_id, &old_account);
		if !is_broken {
			return (weight, old_account);
		}
		info!("Account {account_id} requires recovery");
		let (nonce_weight, nonce) = Self::min_nonce(&account_id);
		weight.saturating_accrue(nonce_weight);
		let account = AccountInfo {
			nonce,
			data: old_account.data,
		};
		(weight, account)
	}

	fn has_broken_nonce(
		account_id: &<T as Config>::AccountId,
		account: &AccountInfo<<T as Config>::Index, <T as Config>::AccountData>,
	) -> (Weight, bool) {
		if !account.nonce.is_zero() || is_precompiled(account_id) {
			// Precompiled contracts in Ethereum usually have nonce = 0. Since precompiled contracts are typically
			// implemented by hooking calls to specific addresses and adding dummy state (to ensure they are callable
			// like regular contracts), there's no need for a non-zero nonce unless they explicitly perform
			// state-changing operations like `CREATE`.
			return (Default::default(), false);
		}
		EP::is_managed_by_evm(account_id)
	}

	fn min_nonce(id: &<T as Config>::AccountId) -> (Weight, <T as Config>::Index) {
		let mut weight = Weight::default();
		let mut nonce = 1u32.into();
		while {
			let contract_id = contract_address(id, nonce);
			let (w, occupied) = EP::is_managed_by_evm(&contract_id);
			weight.saturating_accrue(w);
			occupied
		} {
			nonce += 1u32.into();
		}
		info!("Account {id} minimal valid nonce is {nonce:?}");
		(weight, nonce)
	}
}

fn is_precompiled(address: &H160) -> bool {
	/// The largest precompiled address we currently have by numeric value is 0x900.
	const ZERO_PREFIX_LENGTH: usize = (160 - 16) / 8;
	address.as_bytes()[..ZERO_PREFIX_LENGTH]
		.iter()
		.all(Zero::is_zero)
}

/// Contract address that will be produced by the [`CREATE` opcode][1].
///
/// [1]: https://ethereum.github.io/yellowpaper/paper.pdf#section.7
fn contract_address<N: rlp::Encodable>(sender: &H160, nonce: N) -> H160 {
	let mut rlp = RlpStream::new_list(2);
	rlp.append(sender);
	rlp.append(&nonce);
	/// Address is the rightmost 160 bits of hash.
	const ADDR_OFFSET: usize = (256 - 160) / 8;
	H160::from_slice(&keccak_256(&rlp.out())[ADDR_OFFSET..])
}

#[cfg(test)]
mod test {
	use hex_literal::hex;

	use super::*;

	#[test]
	fn is_precompiled_detects_precompiled_contracts() {
		assert!(is_precompiled(
			&hex!("0000000000000000000000000000000000000900").into(),
		));
		assert!(!is_precompiled(
			&hex!("f803e8ca755ae4770b5e6072a1e3cb97631d76ee").into(),
		));
	}

	#[test]
	fn contract_address_produces_addresses() {
		let addr = contract_address(
			&hex!("f803e8ca755ae4770b5e6072a1e3cb97631d76ee").into(),
			1u32,
		);
		assert_eq!(
			addr,
			hex!("efdd09582498184d14af330e1b02d0c8d63afed5").into(),
		);
	}
}
