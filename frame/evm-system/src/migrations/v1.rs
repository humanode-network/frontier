//! Migration to Version 1.

use frame_support::{log::info, pallet_prelude::*, traits::OnRuntimeUpgrade};

use crate::{Account, AccountInfo, Config, Pallet};

/// The Version 0 account info struct.
#[derive(Decode)]
pub struct CurrentAccountInfo<Index, AccountData> {
	/// The number of transactions this account has sent.
	pub nonce: Index,
	/// The additional data that belongs to this account. Used to store the balance(s) in a lot of
	/// chains.
	pub data: AccountData,
}

/// EVM provider interface.
pub trait EvmProvider<AccountId> {
	/// Check whether account is managed by EVM or not.
	fn is_managed_by_evm(account_id: AccountId) -> bool;
}

/// Execute migration to Version 1 from Version 0.
pub struct MigrationV0ToV1<T, EP>(sp_std::marker::PhantomData<(T, EP)>);

impl<T: Config, EP: EvmProvider<<T as Config>::AccountId>> OnRuntimeUpgrade
	for MigrationV0ToV1<T, EP>
{
	fn on_runtime_upgrade() -> Weight {
		let onchain_version = Pallet::<T>::on_chain_storage_version();

		let mut weight: Weight = T::DbWeight::get().reads(1);

		if onchain_version != 0 {
			info!("Not at version 0, nothing to do. This migrarion probably should be removed");
			return weight;
		}

		info!("Running migration to v1");

		<Account<T>>::translate(
			|account_id,
			 old_account_info: CurrentAccountInfo<
				<T as Config>::Index,
				<T as Config>::AccountData,
			>| {
				let managed_by_evm = EP::is_managed_by_evm(account_id);
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

		info!("Migrated to v1");

		weight
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
		todo!()
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
		todo!()
	}
}
