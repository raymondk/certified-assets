// Draws one button per app from the list the launchpad-backend canister holds.
// That list is whatever the last `on_config_update` call delivered, so the
// buttons follow the config without a redeploy.

import { Actor, HttpAgent, safeGetCanisterEnv } from "./vendor/icp-sdk-core.js";

// How often to ask the backend again, so a config update shows up by itself.
const REFRESH_MS = 3000;

// The backend's interface, as far as this page uses it. Mirrors
// `backend/launchpad-backend.did`.
const idlFactory = ({ IDL }) => {
  const CanisterConfig = IDL.Record({
    domains: IDL.Opt(IDL.Vec(IDL.Text)),
    name: IDL.Opt(IDL.Text),
    icon: IDL.Opt(IDL.Text),
  });
  const ConfigUpdate = IDL.Vec(IDL.Tuple(IDL.Principal, CanisterConfig));
  return IDL.Service({
    list_canisters: IDL.Func([], [ConfigUpdate], ["query"]),
  });
};

const apps = document.getElementById("apps");
const status = document.getElementById("status");

// A local network serves every canister under `localhost`, whatever domains
// the canister has been told about.
const isLocalNetwork =
  location.hostname === "localhost" || location.hostname.endsWith(".localhost");

// The URL a canister is served from by its id, which works whether or not it
// has a domain of its own.
function canisterUrl(canisterId, ownId) {
  const port = location.port ? `:${location.port}` : "";
  if (isLocalNetwork) {
    return `${location.protocol}//${canisterId}.localhost${port}/`;
  }
  // On a gateway that addresses this page by canister id, the same gateway
  // with the id swapped.
  if (ownId && location.hostname.startsWith(`${ownId}.`)) {
    return `${location.protocol}//${canisterId}${location.hostname.slice(ownId.length)}${port}/`;
  }
  // This page is on a custom domain, which says nothing about the gateway.
  return `https://${canisterId}.icp0.io/`;
}

// Where an app's button leads: its first domain, or its canister URL if it has
// none. A local network cannot serve those domains, so there the canister URL
// is always used.
function appUrl(canisterId, domains, ownId) {
  if (domains.length > 0 && !isLocalNetwork) {
    try {
      return new URL(`https://${domains[0]}/`).href;
    } catch {
      // Not a hostname. Fall through to the canister URL.
    }
  }
  return canisterUrl(canisterId, ownId);
}

// Only web URLs are loaded as icons; anything else counts as no icon.
function iconUrl(icon) {
  try {
    const url = new URL(icon);
    return url.protocol === "https:" || url.protocol === "http:" ? url.href : undefined;
  } catch {
    return undefined;
  }
}

function appButton([principal, config], ownId) {
  const canisterId = principal.toText();
  // Candid's `opt` arrives as an empty or one-element array.
  const domains = config.domains[0] ?? [];
  // Without a name, show the next best thing a person would recognise.
  const name = config.name[0] ?? domains[0] ?? canisterId;
  const icon = config.icon[0] && iconUrl(config.icon[0]);

  const button = document.createElement("a");
  button.className = "app";
  button.href = appUrl(canisterId, domains, ownId);
  button.dataset.canisterId = canisterId;

  let picture;
  if (icon) {
    picture = document.createElement("img");
    picture.src = icon;
    picture.alt = "";
  } else {
    picture = document.createElement("span");
    picture.className = "no-icon";
    picture.textContent = name.trim().charAt(0).toUpperCase();
  }

  const label = document.createElement("span");
  label.className = "name";
  label.textContent = name;

  const where = document.createElement("span");
  where.className = "domains";
  where.textContent = domains.length > 0 ? domains.join(", ") : "no custom domain";

  button.append(picture, label, where);
  return button;
}

function render(list, ownId) {
  // The launchpad is in the list too, as one of the canisters with domains,
  // but a button leading back here would be no use.
  const others = list.filter(([principal]) => principal.toText() !== ownId);
  apps.replaceChildren(...others.map((entry) => appButton(entry, ownId)));
  status.textContent =
    others.length > 0
      ? ""
      : "No apps yet. Run ./toggle.sh to send the canisters their config.";
}

async function main() {
  // The asset canister hands the page its environment in the `ic_env` cookie:
  // the network's root key, and the id of every canister in the project.
  const env = safeGetCanisterEnv();
  const backendId = env?.["PUBLIC_CANISTER_ID:launchpad-backend"];
  if (!backendId) {
    status.textContent =
      "This page does not know its backend. Open it from the launchpad canister's URL.";
    return;
  }
  const ownId = env["PUBLIC_CANISTER_ID:launchpad"];

  const agent = await HttpAgent.create({ rootKey: env.IC_ROOT_KEY });
  const backend = Actor.createActor(idlFactory, { agent, canisterId: backendId });

  const refresh = async () => {
    try {
      render(await backend.list_canisters(), ownId);
    } catch (error) {
      console.error(error);
      status.textContent = "Could not reach the launchpad backend.";
    }
  };
  await refresh();
  setInterval(refresh, REFRESH_MS);
}

main();
