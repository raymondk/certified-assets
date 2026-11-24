# Config update (several canisters, one config)

Three sites and a backend that all learn their configuration from the same
`on_config_update` call. A launchpad page shows one button per app, and the names,
icons and domains on those buttons change when the config does, with no redeploy.

It is exercised in CI by
[`crates/e2e/tests/config_update.rs`](../../crates/e2e/tests/config_update.rs), which
deploys this project to a local replica and runs `toggle.sh` twice.

## What it demonstrates

- **One argument for every canister.** `on_config_update` takes a map from canister
  ID to that canister's `domains`, `name` and `icon`. The same map is sent unchanged
  to all four canisters, and each one uses the part it needs.
- **What an asset canister does with it.** `openhr`, `openteam` and `launchpad` each
  look up their own entry and write its `domains` to
  `/.well-known/ii-alternative-origins` and `/.well-known/ic-domains`. They ignore
  `name` and `icon`.
- **What another kind of canister can do with it.** `launchpad-backend` is a small
  Rust canister with its own `on_config_update`. It keeps the whole map and serves it
  from a `list_canisters` query, which the launchpad page reads to draw its buttons.
- **Keeping the written files across deploys.** Each site lists the two well-known
  files in [`_preserve`](../../docs/site-files.md#listing-files-in-_preserve), since
  no build ships them and a deploy would otherwise delete them.
- **Finding a backend from a static page.** The launchpad page has no build step. It
  reads the backend's canister ID from the `ic_env` cookie the asset canister sets.

## Project structure

```
config-update
├── icp.yaml              # the four canisters
├── toggle.sh             # sends one of two preset configs to all of them
├── openhr                # site: a page reading "openhr canister"
├── openteam              # site: a page reading "openteam canister"
├── launchpad             # site: one button per app
│   ├── launchpad.js      # queries the backend and draws the buttons
│   └── vendor            # the ICP JavaScript agent, pre-bundled
└── backend               # the launchpad-backend canister (Rust)
    ├── launchpad-backend.did
    └── src/lib.rs
```

## Prerequisites

- [icp-cli](https://cli.internetcomputer.org)
- A Rust toolchain with the `wasm32-unknown-unknown` and `wasm32-wasip2` targets
  (to build the canisters + plugin), plus `make`.
- `bash` and `curl`, for `toggle.sh`.

## Run it

From the repo root, build the asset canister and sync-plugin wasms. This example's
`icp.yaml` references them at `../../dist/`:

```sh
make wasm
```

Deploy to a local replica. This also builds the backend canister:

```sh
cd examples/config-update
icp network start -d
icp deploy
```

Open the `launchpad` URL that `icp deploy` prints. It has no buttons yet, because
nothing has sent the canisters a config. Send one:

```sh
./toggle.sh
```

The launchpad now shows an **OpenHR** and an **OpenTeam** button, each with an icon
and its domains. The page checks for changes every few seconds, so there is no need to
reload. A button opens that app's page.

Run the script again to send the other preset:

```sh
./toggle.sh
```

The buttons become **People & HR** and **Team Directory**. Each further run switches
back and forth. `./toggle.sh a` and `./toggle.sh b` send a specific preset.

The script also prints each site's `/.well-known/ic-domains` after every run, so you
can see the asset canisters follow the same config. To look for yourself, open
`/.well-known/ii-alternative-origins` or `/.well-known/ic-domains` on any of the three
sites.

Stop the replica when you're done:

```sh
icp network stop
```

## The two presets

| Canister | Preset `a` | Preset `b` |
|---|---|---|
| `openhr` | "OpenHR", `openhr.example.com`, an icon | "People & HR", `people.example.org`, another icon |
| `openteam` | "OpenTeam", `openteam.example.com` and `team.example.com`, an icon | "Team Directory", no domains, no icon |
| `launchpad` | "Launchpad", `launchpad.example.com`, no icon | "App Launcher", `apps.example.org`, no icon |

Preset `b` gives `openteam` an empty domain list, which empties its two files, and no
icon, which the launchpad draws a placeholder for.

The domains are placeholders that do not resolve. A button normally leads to its app's
first domain. On a local network, where no custom domain can work, it leads to the
app's canister URL instead, so the buttons work in this demo. The icons are SVG files
that `openhr` and `openteam` serve themselves.

## The argument

`toggle.sh` builds this value, with the three canister IDs filled in, and passes it to
`on_config_update` on every canister:

```candid
(vec {
  record { principal "<openhr>"; record {
    domains = opt vec { "openhr.example.com" };
    name = opt "OpenHR";
    icon = opt "http://<openhr>.localhost:<port>/icon.svg";
  } };
  record { principal "<openteam>"; record {
    domains = opt vec { "openteam.example.com"; "team.example.com" };
    name = opt "OpenTeam";
    icon = opt "http://<openteam>.localhost:<port>/icon.svg";
  } };
  record { principal "<launchpad>"; record {
    domains = opt vec { "launchpad.example.com" };
    name = opt "Launchpad";
    icon = null;
  } };
})
```

All three fields are optional. See
[Sending a config update](../../docs/site-files.md#sending-a-config-update) for what an
asset canister does with each form of `domains`.

`launchpad-backend` has no entry. It is not one of the apps, and unlike an asset
canister it does not need to find itself in the map.

Every call in the script passes `--candid` with the canister's interface file. The
wasms this example deploys do not embed their interface, and without one
`icp canister call` infers the argument's types from its text. It gets them wrong when
entries differ in shape, such as one with `icon = opt "..."` beside one with
`icon = null`, and the canister then rejects the call. A canister deployed from the
published recipe embeds its interface, so calls to it need no `--candid`.

## The vendored agent

`launchpad/vendor/icp-sdk-core.js` is the ICP JavaScript SDK
([`@icp-sdk/core`](https://www.npmjs.com/package/@icp-sdk/core) 6.1.1), bundled into
one file so that the example needs no Node.js and no build step. It is generated, not
written by hand. To rebuild it, in an empty directory:

```sh
npm install @icp-sdk/core@6.1.1 esbuild
cat > entry.js <<'JS'
export { Actor, HttpAgent } from "@icp-sdk/core/agent";
export { safeGetCanisterEnv } from "@icp-sdk/core/agent/canister-env";
JS
npx esbuild entry.js --bundle --format=esm --minify --target=es2022 \
  --legal-comments=inline --outfile=icp-sdk-core.js
```

Then put the comment line from the top of the current file back at the top of the new
one, with the versions updated. A real project would install the SDK with its package
manager and let its bundler do this.
