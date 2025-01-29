//! Migration to Version 1.

#[cfg(feature = "try-runtime")]
use frame_support::sp_std::{vec, vec::Vec};
use frame_support::{log::info, pallet_prelude::*, storage_alias, traits::OnRuntimeUpgrade};

use crate::{Account, AccountInfo, Config, Pallet};

/// The Version 0 account info struct.
#[derive(Default, Decode, Encode)]
pub struct AccountInfoV0<Index, AccountData> {
	/// The number of transactions this account has sent.
	pub nonce: Index,
	/// The additional data that belongs to this account. Used to store the balance(s) in a lot of
	/// chains.
	pub data: AccountData,
}

/// EVM provider interface.
pub trait EvmProvider<AccountId> {
	/// Check whether account is managed by EVM or not.
	fn is_managed_by_evm(account_id: &AccountId) -> bool;
}

/// Execute migration to Version 1 from Version 0.
pub struct MigrationV0ToV1<EP, T>(sp_std::marker::PhantomData<(EP, T)>);

impl<EP: EvmProvider<<T as Config>::AccountId>, T: Config> OnRuntimeUpgrade
	for MigrationV0ToV1<EP, T>
{
	fn on_runtime_upgrade() -> Weight {
		let onchain_version = Pallet::<T>::on_chain_storage_version();
		let pallet_name = Pallet::<T>::name();

		let mut weight: Weight = T::DbWeight::get().reads(1);

		if onchain_version != 0 {
			info!(
				"{}: Not at version 0, nothing to do. This migrarion probably should be removed",
				pallet_name,
			);
			return weight;
		}

		info!("{}: Running migration to v1", pallet_name);

		<Account<T>>::translate::<AccountInfoV0<<T as Config>::Index, <T as Config>::AccountData>, _>(
			|account_id, old_account_info| {
				let managed_by_evm = EP::is_managed_by_evm(&account_id);
				let account_info = AccountInfo::<_, _> {
					nonce: old_account_info.nonce,
					managed_by_evm,
					data: old_account_info.data,
				};

				weight.saturating_accrue(T::DbWeight::get().reads_writes(1, 1));
				Some(account_info)
			},
		);

		// Set storage version to `1`.
		StorageVersion::new(1).put::<Pallet<T>>();
		weight.saturating_accrue(T::DbWeight::get().writes(1));

		info!("{}: Migrated to v1", pallet_name);

		weight
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
		/// The Version 0 account storage.
		#[storage_alias]
		type Account<T: Config> = StorageMap<
			Pallet<T>,
			Blake2_128Concat,
			<T as Config>::AccountId,
			AccountInfoV0<<T as Config>::Index, <T as Config>::AccountData>,
			ValueQuery,
		>;

		let onchain = <Pallet<T>>::on_chain_storage_version();

		// Disable the check for newer versions by returning an empty state.
		if onchain >= 1 {
			return Ok(vec![]);
		}

		let pre_count: u64 = <Account<T>>::iter().count().try_into().unwrap();

		Ok(pre_count.encode())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(state: Vec<u8>) -> Result<(), &'static str> {
		// Empty state means that the check is disabled.
		if state.is_empty() {
			return Ok(());
		}

		// Ensure version is updated correctly.
		let onchain = <Pallet<T>>::on_chain_storage_version();
		assert_eq!(onchain, 1);

		// Ensure the accounts count matches.
		let pre_count: u64 = scale_codec::Decode::decode(&mut &*state).unwrap();
		let post_count: u64 = Account::<T>::iter().count().try_into().unwrap();
		assert_eq!(pre_count, post_count);

		// Ensure storage data is updated correctly.
		Account::<T>::iter().for_each(|(_account_id, account_info)| {
			assert!(
				account_info.managed_by_evm == true || account_info.managed_by_evm == false,
				"none of accounts should be in destroying status, or undefined state"
			)
		});

		Ok(())
	}
}
