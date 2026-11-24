//! `on_config_update`, end to end through the `config-update` example: one
//! argument describing a group of canisters, sent unchanged to each of them.
//! The asset canisters write their own domain files from it; the example's
//! backend canister keeps the whole map for the launchpad page.

use candid::Principal;
use e2e::{
    LocalNetwork, canister_id, committed_doc, documented_calls, gateway_url, icp_cmd, setup_example,
};
use reqwest::StatusCode;
use std::path::Path;
use std::process::Command;
use wire_types::CanisterConfig;

const ORIGINS: &str = "/.well-known/ii-alternative-origins";
const IC_DOMAINS: &str = "/.well-known/ic-domains";

/// The interface of the asset canisters. The wasm the tests deploy carries
/// none, and without one `icp canister call` infers the argument's types from
/// its text.
const ASSETS_DID: &str = "../../certified-assets.did";

/// The principal `docs/site-files.md` uses in its `on_config_update` call.
const DOCUMENTED_PRINCIPAL: &str = "ryjl3-tyaaa-aaaaa-aaaba-cai";

/// Fetch `path` from the canister called `name`, through the gateway.
fn fetch(project: &Path, name: &str, path: &str) -> reqwest::blocking::Response {
    let url = format!(
        "{}{path}?canisterId={}",
        gateway_url(project),
        canister_id(project, name)
    );
    reqwest::blocking::get(&url).unwrap_or_else(|e| panic!("GET {url} failed: {e}"))
}

fn body(project: &Path, name: &str, path: &str) -> String {
    let response = fetch(project, name, path);
    assert_eq!(response.status(), StatusCode::OK, "{name}{path}");
    response.text().unwrap()
}

/// Runs the example's own script, the way its README tells a reader to.
fn toggle(project: &Path) {
    let output = Command::new(project.join("toggle.sh"))
        .current_dir(project)
        .output()
        .expect("failed to run toggle.sh");
    assert!(
        output.status.success(),
        "toggle.sh failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

/// What the launchpad page reads: the map the backend canister holds.
fn list_canisters(project: &Path) -> Vec<(Principal, CanisterConfig)> {
    let stdout = icp_cmd(project)
        .args([
            "canister",
            "call",
            "--query",
            "launchpad-backend",
            "list_canisters",
            "()",
            "-o",
            "hex",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let bytes = hex::decode(String::from_utf8_lossy(&stdout).trim()).expect("hex reply");
    candid::decode_one(&bytes).expect("failed to decode the list_canisters reply")
}

/// The entry `list` holds for the canister called `name`.
fn entry(project: &Path, list: &[(Principal, CanisterConfig)], name: &str) -> CanisterConfig {
    let id = Principal::from_text(canister_id(project, name)).unwrap();
    list.iter()
        .find(|(principal, _)| *principal == id)
        .unwrap_or_else(|| panic!("no entry for {name}"))
        .1
        .clone()
}

fn names(domains: &[&str]) -> Option<Vec<String>> {
    Some(domains.iter().map(|d| d.to_string()).collect())
}

/// Deploy the example, flip its config back and forth with `toggle.sh`, and
/// check that every receiver followed: each asset canister serves its own
/// domains in both well-known files, and the backend lists every app.
#[test]
fn toggling_the_config_updates_every_canister() {
    let tmp = setup_example("config-update");
    let project = tmp.path();
    let _network = LocalNetwork::start(project);
    icp_cmd(project).arg("deploy").assert().success();

    // Nothing has sent a config yet.
    assert!(list_canisters(project).is_empty());
    assert_eq!(
        fetch(project, "openhr", IC_DOMAINS).status(),
        StatusCode::NOT_FOUND
    );

    // First run: preset a.
    toggle(project);
    let list = list_canisters(project);
    assert_eq!(list.len(), 3, "openhr, openteam and launchpad");
    let openhr = entry(project, &list, "openhr");
    assert_eq!(openhr.name.as_deref(), Some("OpenHR"));
    assert_eq!(openhr.domains, names(&["openhr.example.com"]));
    assert!(openhr.icon.is_some_and(|icon| icon.ends_with("/icon.svg")));
    assert_eq!(
        entry(project, &list, "openteam").name.as_deref(),
        Some("OpenTeam")
    );
    assert_eq!(
        body(project, "openhr", ORIGINS),
        r#"{"alternativeOrigins":["https://openhr.example.com"]}"#
    );
    assert_eq!(body(project, "openhr", IC_DOMAINS), "openhr.example.com\n");
    assert_eq!(
        body(project, "openteam", ORIGINS),
        r#"{"alternativeOrigins":["https://openteam.example.com","https://team.example.com"]}"#
    );
    assert_eq!(
        body(project, "openteam", IC_DOMAINS),
        "openteam.example.com\nteam.example.com\n"
    );
    assert_eq!(
        body(project, "launchpad", IC_DOMAINS),
        "launchpad.example.com\n"
    );

    // Second run: the script sees preset a is live and sends preset b.
    toggle(project);
    let list = list_canisters(project);
    let openhr = entry(project, &list, "openhr");
    assert_eq!(openhr.name.as_deref(), Some("People & HR"));
    assert!(
        openhr
            .icon
            .is_some_and(|icon| icon.ends_with("/icon-alt.svg"))
    );
    let openteam = entry(project, &list, "openteam");
    assert_eq!(openteam.name.as_deref(), Some("Team Directory"));
    assert_eq!(openteam.domains, Some(vec![]));
    assert_eq!(openteam.icon, None);
    assert_eq!(
        body(project, "openhr", ORIGINS),
        r#"{"alternativeOrigins":["https://people.example.org"]}"#
    );
    assert_eq!(body(project, "openhr", IC_DOMAINS), "people.example.org\n");
    // An empty list empties both of openteam's files.
    assert_eq!(
        body(project, "openteam", ORIGINS),
        r#"{"alternativeOrigins":[]}"#
    );
    assert_eq!(body(project, "openteam", IC_DOMAINS), "");
    assert_eq!(body(project, "launchpad", IC_DOMAINS), "apps.example.org\n");

    // The sites list both files in `_preserve`, so a deploy keeps them even
    // though no build ships them.
    icp_cmd(project).arg("deploy").assert().success();
    assert_eq!(body(project, "openhr", IC_DOMAINS), "people.example.org\n");
    assert_eq!(
        body(project, "openhr", ORIGINS),
        r#"{"alternativeOrigins":["https://people.example.org"]}"#
    );

    // An entry that says nothing about domains leaves the files alone.
    let openhr_id = canister_id(project, "openhr");
    icp_cmd(project)
        .args(["canister", "call", "--candid", ASSETS_DID, "openhr"])
        .arg("on_config_update")
        .arg(format!(
            r#"(vec {{ record {{ principal "{openhr_id}"; record {{ name = opt "Renamed" }} }} }})"#
        ))
        .assert()
        .success();
    assert_eq!(body(project, "openhr", IC_DOMAINS), "people.example.org\n");

    // The call `docs/site-files.md` shows, with the canister's real id in
    // place of the documentation's. Running the committed snippet is what
    // keeps the documented call form from drifting from the interface.
    let calls = documented_calls(&committed_doc("docs/site-files.md"), &["on_config_update"]);
    assert_eq!(calls.len(), 1, "expected one documented on_config_update");
    let (method, args) = &calls[0];
    assert!(
        args.contains(DOCUMENTED_PRINCIPAL),
        "the documented call no longer names {DOCUMENTED_PRINCIPAL}: {args}"
    );
    icp_cmd(project)
        .args(["canister", "call", "--candid", ASSETS_DID, "openhr", method])
        .arg(args.replace(DOCUMENTED_PRINCIPAL, &openhr_id))
        .assert()
        .success();
    assert_eq!(
        body(project, "openhr", IC_DOMAINS),
        "example.com\nwww.example.com\n"
    );

    // Sent as written, the documented call names some other canister, and a
    // canister the map does not name refuses it.
    let refused = icp_cmd(project)
        .args(["canister", "call", "--candid", ASSETS_DID, "openhr", method])
        .arg(args)
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&refused.get_output().stderr).to_string();
    assert!(
        stderr.contains("no entry for this canister"),
        "expected the canister's refusal; got:\n{stderr}"
    );
    assert_eq!(
        body(project, "openhr", IC_DOMAINS),
        "example.com\nwww.example.com\n"
    );
}
