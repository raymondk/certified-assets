use candid::Principal;
use canister_core::{
    AssetDetails, ByteBuf, CanisterConfig, ChunkId, ExecuteOperationsArguments, HttpRequest,
    HttpResponse, InitArgs, IssueTokenArgs, ProposedState, ProtectionStatus, RedirectRule,
    StartSyncResult, TokenInfo, UploadAssetArguments, UploadChunksArguments, Version,
    guard_can_sync, guard_is_controller, guard_is_governance,
};
use ic_cdk::{init, post_upgrade, query, update};

// Runs on install and reinstall. Optional, so an ordinary `icp deploy` passes
// nothing and gets an unconfigured canister; an SNS reinstalling a governed
// frontend in place passes the approver and deploy principals here, because
// after the install there is no principal left who could set them.
#[init]
fn init(args: Option<InitArgs>) {
    canister_core::init(args);
}

#[post_upgrade]
fn post_upgrade() {
    canister_core::post_upgrade();
}

#[cfg(target_family = "wasm")]
#[used]
#[unsafe(link_section = "icp:public supported_certificate_versions")]
static CERTIFICATE_VERSIONS: [u8; 1] = *b"2"; // The canister supports v2 certificates, but not v1.

// Query methods

#[query]
fn version() -> Version {
    canister_core::version()
}

#[query]
fn get_asset_details(start_after: Option<String>) -> Vec<AssetDetails> {
    canister_core::get_asset_details(start_after)
}

#[query]
fn http_request(req: HttpRequest) -> HttpResponse {
    canister_core::http_request(req)
}

#[query]
fn get_redirect_rules(start_index: u64) -> Vec<RedirectRule> {
    canister_core::get_redirect_rules(start_index)
}

// Update methods

#[update(guard = "guard_is_controller")]
fn authorize(principal: Principal) {
    canister_core::authorize(principal)
}

#[update(guard = "guard_is_controller")]
fn deauthorize(principal: Principal) {
    canister_core::deauthorize(principal)
}

// Validators for the two methods above, so a DAO can manage its syncing
// principals by proposal: a generic nervous system function is rejected without
// a `validator_method_name`. Read-only, unguarded, and updates rather than
// queries because an inter-canister call cannot reach a query entry point.

#[update]
fn validate_authorize(principal: Principal) -> Result<String, String> {
    canister_core::validate_authorize(principal)
}

#[update]
fn validate_deauthorize(principal: Principal) -> Result<String, String> {
    canister_core::validate_deauthorize(principal)
}

// Recaptures the `PUBLIC_*` env vars + IC root key and re-certifies the `ic_env`
// cookie onto every HTML response. Any sync-authorized caller may trigger it
// (same guard as a sync) — only controllers can *change* the env vars, but an
// authorized syncer should be able to re-publish them. The env is also captured
// at the start of every sync, so this is only needed to publish an env-var
// change without a content sync.
#[update(guard = "guard_can_sync")]
fn refresh_env() {
    canister_core::refresh_env()
}

#[update]
fn list_authorized() -> Vec<Principal> {
    canister_core::list_authorized()
}

// The canister's canonical state hash: a SHA-256 over its served-content model
// (every asset's content_type/headers/encoding hashes + the redirect rules; see
// `state-hash`). Recomputed at the end of every final sync. Public and
// unguarded; an **update** (not a query) so the reply is consensus-backed and a
// third party can trust it against a hash they computed locally from the source
// build. Returns the cached value with no recomputation.
#[update]
fn state_hash() -> ByteBuf {
    canister_core::state_hash()
}

// The compression fingerprint of the client that last prepared every asset (see
// `asset-prep::canary`), or 32 zero bytes if none has. A syncing client reads it
// to decide whether it may trust unchanged uncompressed hashes to imply
// unchanged compressed bytes. A **query**: it only steers the client's own local
// work, so a stale read costs at worst some redundant preparation, never
// incorrect state — unlike `state_hash`, which a third party trusts.
#[query]
fn preparation_canary() -> ByteBuf {
    canister_core::preparation_canary()
}

// Whether the calling identity may sync assets (authorized or a controller).
// Lets a client check access up front instead of discovering it mid-sync.
#[query]
fn can_sync() -> bool {
    canister_core::can_sync()
}

#[update(guard = "guard_can_sync")]
fn start_sync() -> StartSyncResult {
    canister_core::start_sync()
}

#[update(guard = "guard_can_sync")]
fn upload_chunks(arg: UploadChunksArguments) -> Vec<ChunkId> {
    canister_core::upload_chunks(arg)
}

// Returns the canonical state hash on the call flagged `is_final` (`None`
// otherwise), so a finishing sync learns what it installed without a second
// round trip. Canister-reported: a self-consistency read, not verification.
#[update(guard = "guard_can_sync")]
async fn execute_operations(arg: ExecuteOperationsArguments) -> Option<ByteBuf> {
    canister_core::execute_operations(arg).await
}

// Writes one asset outside a sync. Discouraged — see `docs/site-files.md`:
// the asset is not in any build, so the state hash stops reproducing from
// source, and the next sync deletes it unless `_preserve` names it. Refused in
// by-proposal mode and while a sync is in progress.
#[update(guard = "guard_can_sync")]
async fn upload_asset(arg: UploadAssetArguments) -> ByteBuf {
    canister_core::upload_asset(arg).await
}

// Tells the canister the configuration of its group of canisters changed. The
// argument maps canister ids to their configuration, so one payload can be sent
// unchanged to many canisters; a canister it does not name exactly once traps.
// Today the reaction is to write two files from the entry's `domains`, in one
// sync: `/.well-known/ii-alternative-origins` for Internet Identity and
// `/.well-known/ic-domains` for the boundary nodes. An entry with no `domains`
// writes nothing, and `name` and `icon` are ignored. Same caveats as
// `upload_asset`. Named for the event rather than those actions because more
// reactions are expected. Returns nothing and traps on failure, so a caller can
// treat every kind of receiver alike.
#[update(guard = "guard_can_sync")]
async fn on_config_update(config: Vec<(Principal, CanisterConfig)>) {
    canister_core::on_config_update(config).await
}

// ───────── Access protection ─────────
// Controller-only configuration for the "private app" gate. None of these touch
// the serving hot path; all serving (including login validation) stays in the
// `http_request` query.

#[update(guard = "guard_is_controller")]
fn enable_protection(login_page: String) {
    canister_core::enable_protection(login_page)
}

#[update(guard = "guard_is_controller")]
fn disable_protection() {
    canister_core::disable_protection()
}

#[update(guard = "guard_is_controller")]
async fn issue_token(args: IssueTokenArgs) -> String {
    canister_core::issue_token(args).await
}

#[update(guard = "guard_is_controller")]
fn revoke_token(label: String) {
    canister_core::revoke_token(label)
}

#[query(guard = "guard_is_controller")]
fn list_tokens() -> Vec<TokenInfo> {
    canister_core::list_tokens()
}

#[query(guard = "guard_is_controller")]
fn check_protection_status() -> ProtectionStatus {
    canister_core::check_protection_status()
}

// ───────── Governance mode (by-proposal deploys) ─────────
// Off unless an approver is set. With one, a sync prepares instead of
// publishing, and only the approver — an SNS/NNS governance canister executing
// an adopted proposal — can make the prepared state live.

#[query]
fn governance() -> Option<Principal> {
    canister_core::governance()
}

#[update(guard = "guard_is_controller")]
fn set_governance(approver: Option<Principal>) {
    canister_core::set_governance(approver)
}

// The validator for `set_governance` as an SNS generic function, so a DAO can
// rotate or clear its own approver by proposal. Read-only and unguarded, and an
// update for the same reason as the other validators.
#[update]
fn validate_set_governance(approver: Option<Principal>) -> Result<String, String> {
    canister_core::validate_set_governance(approver)
}

#[query]
fn proposed_state() -> ProposedState {
    canister_core::proposed_state()
}

// The target method of the SNS generic function. Traps on any failure: SNS
// governance discards the reply, so an `Err` return would be recorded as a
// successful execution. A trap is both the only failure governance surfaces and
// what rolls the whole commit back, leaving the canister untouched.
#[update(guard = "guard_is_governance")]
fn commit_proposed_state(state_hash_hex: String) {
    canister_core::commit_proposed_state(state_hash_hex)
}

// The generic function's validator, rendering the payload for voters. An update
// rather than a query because an inter-canister call cannot reach a query entry
// point, and unguarded because governance calls it while merely validating.
#[update]
fn validate_commit_proposed_state(state_hash_hex: String) -> Result<String, String> {
    canister_core::validate_commit_proposed_state(state_hash_hex)
}

// Guarded like a sync, so the developer who prepared a batch can clear it after
// a rejected proposal without needing another one.
#[update(guard = "guard_can_sync")]
fn discard_proposed_state() {
    canister_core::discard_proposed_state()
}

ic_cdk::export_candid!();

#[test]
fn candid_interface_compatibility() {
    use candid_parser::utils::{CandidSource, service_equal};
    use std::path::PathBuf;

    let new_interface = __export_service();

    // crates/canister -> crates -> workspace root, then certified-assets.did
    let old_interface = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("../../certified-assets.did");

    println!("Exported interface: {new_interface}");

    service_equal(
        CandidSource::Text(&new_interface),
        CandidSource::File(old_interface.as_path()),
    )
    .expect("The assets canister interface is not compatible with the certified-assets.did file");
}

/// Encodes `args`, written in Candid's text form, as `on_config_update`'s
/// argument the way a caller holding `certified-assets.did` does, and decodes
/// the bytes as the canister does.
#[cfg(test)]
fn decode_config_update(args: &str) -> Vec<(Principal, CanisterConfig)> {
    use candid_parser::utils::CandidSource;
    use std::path::PathBuf;

    let did = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("../../certified-assets.did");
    let (env, actor) = CandidSource::File(did.as_path()).load().unwrap();
    let method = env
        .get_method(&actor.unwrap(), "on_config_update")
        .unwrap()
        .clone();
    let bytes = candid_parser::parse_idl_args(args)
        .unwrap()
        .to_bytes_with_types(&env, &method.args)
        .unwrap();
    candid::decode_one(&bytes).unwrap()
}

/// The argument is an ordinary typed Candid value, a map from canister id to
/// a record whose fields are all optional.
#[test]
fn on_config_update_argument_decodes_from_its_candid_text_form() {
    let openhr = Principal::from_text("uxrrr-q7777-77774-qaaaq-cai").unwrap();
    let openteam = Principal::from_text("u6s2n-gx777-77774-qaaba-cai").unwrap();
    let launchpad = Principal::from_text("uzt4z-lp777-77774-qaabq-cai").unwrap();

    let decoded = decode_config_update(
        r#"(vec {
          record { principal "uxrrr-q7777-77774-qaaaq-cai"; record {
            domains = opt vec { "openhr.example.com"; "hr.example.com" };
            name = opt "OpenHR";
            icon = opt "https://openhr.example.com/icon.png";
          } };
          record { principal "u6s2n-gx777-77774-qaaba-cai"; record {
            domains = opt vec {};
            name = null;
            icon = null;
          } };
          record { principal "uzt4z-lp777-77774-qaabq-cai"; record {} };
        })"#,
    );

    assert_eq!(
        decoded,
        vec![
            (
                openhr,
                CanisterConfig {
                    domains: Some(vec![
                        "openhr.example.com".to_string(),
                        "hr.example.com".to_string()
                    ]),
                    name: Some("OpenHR".to_string()),
                    icon: Some("https://openhr.example.com/icon.png".to_string()),
                }
            ),
            (
                openteam,
                CanisterConfig {
                    // Stated as empty, which is not the same as left out.
                    domains: Some(vec![]),
                    name: None,
                    icon: None,
                }
            ),
            // Every field left out: each reads as not stated.
            (launchpad, CanisterConfig::default()),
        ]
    );
}

/// A sender whose own types make a field required (`domains : vec text`,
/// `name : text`) puts the bare value on the wire. Candid reads a value as an
/// `opt` of its type, so that sender and this canister still agree.
#[test]
fn on_config_update_argument_accepts_fields_sent_as_required() {
    // Encoded without the interface, so the fields keep the types written here.
    let bytes = candid_parser::parse_idl_args(
        r#"(vec { record { principal "uxrrr-q7777-77774-qaaaq-cai"; record {
          domains = vec { "example.com" };
          name = "OpenHR";
        } } })"#,
    )
    .unwrap()
    .to_bytes()
    .unwrap();

    let decoded: Vec<(Principal, CanisterConfig)> = candid::decode_one(&bytes).unwrap();
    assert_eq!(
        decoded[0].1,
        CanisterConfig {
            domains: Some(vec!["example.com".to_string()]),
            name: Some("OpenHR".to_string()),
            icon: None,
        }
    );
}

/// The price of an optional field: Candid reads a value it cannot fit into an
/// `opt` as `null` instead of failing. A sender that gets `domains` wrong is
/// therefore not told so. Its entry reads as saying nothing about domains,
/// which leaves the files alone.
#[test]
fn on_config_update_argument_reads_a_mistyped_optional_field_as_null() {
    let bytes = candid_parser::parse_idl_args(
        r#"(vec { record { principal "uxrrr-q7777-77774-qaaaq-cai"; record {
          domains = "example.com";
          name = opt "OpenHR";
        } } })"#,
    )
    .unwrap()
    .to_bytes()
    .unwrap();

    let decoded: Vec<(Principal, CanisterConfig)> = candid::decode_one(&bytes).unwrap();
    assert_eq!(
        decoded[0].1,
        CanisterConfig {
            domains: None,
            name: Some("OpenHR".to_string()),
            icon: None,
        }
    );
}
