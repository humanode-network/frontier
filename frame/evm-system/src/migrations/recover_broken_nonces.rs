//! Migration to recover broken nonces.

#[cfg(feature = "try-runtime")]
use frame_support::sp_std::{vec, vec::Vec};
use frame_support::{log::info, pallet_prelude::*, storage_alias, traits::OnRuntimeUpgrade};

use crate::{Account, AccountInfo, Config, Pallet};

/// EVM provider interface.
pub trait EvmProvider<AccountId> {
	/// Check whether account is managed by EVM or not.
	fn is_managed_by_evm(account_id: &AccountId) -> bool;
}

/// Execute migration to recover broken nonces.
pub struct MigrationBrokenNoncesRecover<EP, T>(sp_std::marker::PhantomData<(EP, T)>);

impl<EP: EvmProvider<<T as Config>::AccountId>, T: Config> OnRuntimeUpgrade
	for MigrationBrokenNoncesRecover<EP, T>
{
	fn on_runtime_upgrade() -> Weight {
		info!("{}: Running migration to recover broken nonces", pallet_name);

		// TODO: implement a logic to recover broken nonces.
		todo!()

		info!("{}: Migrated", pallet_name);

		weight
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
		// TODO: some checks before migration.
		todo!()
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(state: Vec<u8>) -> Result<(), &'static str> {
		// TODO: some checks after migration.
		todo!()
	}
}
