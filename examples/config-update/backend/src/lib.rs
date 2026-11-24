//! The launchpad's backend canister.
//!
//! It receives the same `on_config_update` argument the asset canisters do, but
//! uses it differently: where an asset canister picks out its own entry and
//! writes its domain files, this canister keeps the whole map, so the launchpad
//! page can list every app with its name, icon and domains.

use candid::{CandidType, Principal};
use ic_cdk::api::{is_controller, msg_caller, trap};
use ic_cdk::{post_upgrade, pre_upgrade, query, update};
use serde::Deserialize;
use std::cell::RefCell;
use std::collections::BTreeSet;

/// One canister's entry in a config update. Mirrors `CanisterConfig` in
/// `certified-assets.did`: every field is optional, and a receiver reads only
/// the ones it uses.
#[derive(Clone, Debug, PartialEq, Eq, CandidType, Deserialize)]
pub struct CanisterConfig {
    pub domains: Option<Vec<String>>,
    pub name: Option<String>,
    pub icon: Option<String>,
}

/// A map from canister id to that canister's config, kept as a list so the
/// launchpad shows the apps in the order they were sent.
type ConfigUpdate = Vec<(Principal, CanisterConfig)>;

thread_local! {
    static CONFIG: RefCell<ConfigUpdate> = const { RefCell::new(Vec::new()) };
}

/// Replaces the stored map. Unlike an asset canister, this one needs no entry
/// of its own: it is the listing, not one of the apps listed.
#[update]
fn on_config_update(config: ConfigUpdate) {
    if !is_controller(&msg_caller()) {
        trap("only a controller may update the config");
    }
    let mut seen = BTreeSet::new();
    for (canister_id, _) in &config {
        if !seen.insert(*canister_id) {
            trap(format!("more than one entry for canister {canister_id}"));
        }
    }
    CONFIG.with_borrow_mut(|stored| *stored = config);
}

#[query]
fn list_canisters() -> ConfigUpdate {
    CONFIG.with_borrow(Clone::clone)
}

// A redeploy upgrades the canister, which wipes its heap. Carrying the map
// across keeps the launchpad populated until the next update arrives.

#[pre_upgrade]
fn pre_upgrade() {
    CONFIG
        .with_borrow(|stored| ic_cdk::storage::stable_save((stored,)))
        .unwrap_or_else(|e| trap(format!("failed to save the config: {e}")));
}

#[post_upgrade]
fn post_upgrade() {
    let (stored,): (ConfigUpdate,) = ic_cdk::storage::stable_restore()
        .unwrap_or_else(|e| trap(format!("failed to restore the config: {e}")));
    CONFIG.with_borrow_mut(|config| *config = stored);
}

ic_cdk::export_candid!();

#[test]
fn candid_interface_matches_the_did_file() {
    use candid_parser::utils::{CandidSource, service_equal};
    use std::path::PathBuf;

    let did =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("launchpad-backend.did");
    service_equal(
        CandidSource::Text(&__export_service()),
        CandidSource::File(did.as_path()),
    )
    .expect("the canister's interface differs from launchpad-backend.did");
}
