#!/usr/bin/env bash
# Sends one of two preset configs to every canister in this example, by calling
# `on_config_update` on each with the same argument. Run it again to send the
# other preset.
#
#   ./toggle.sh        # switch to the preset that is not live
#   ./toggle.sh a      # send preset a
#   ./toggle.sh b      # send preset b
#
# Run `icp network start -d` and `icp deploy` first; see README.md.
set -euo pipefail
cd "$(dirname "$0")"

# The wasms this example deploys carry no Candid interface, and without one
# `icp canister call` guesses the argument's types from its text. That guess is
# wrong for a list whose entries differ in shape, such as one `icon = opt "..."`
# next to one `icon = null`, so every call names the interface.
ASSETS_DID=../../certified-assets.did
BACKEND_DID=backend/launchpad-backend.did

openhr=$(icp canister status openhr --id-only)
openteam=$(icp canister status openteam --id-only)
launchpad=$(icp canister status launchpad --id-only)

# The local gateway serves each canister at `<canister-id>.localhost:<port>`.
# The icons are files the canisters themselves serve, so their URLs need it.
gateway=$(icp network status --json | sed -n 's/.*"gateway_url" *: *"\([^"]*\)".*/\1/p')
gateway=${gateway%/}
if [[ -z "$gateway" ]]; then
  echo "could not read the gateway URL from \`icp network status --json\`" >&2
  exit 1
fi
url() { echo "${gateway/:\/\//://$1.}"; }

preset=${1:-}
if [[ -z "$preset" ]]; then
  # The backend holds the last config sent, so it knows which preset is live.
  current=$(icp canister call --candid "$BACKEND_DID" --query launchpad-backend list_canisters '()')
  if [[ "$current" == *'"OpenHR"'* ]]; then preset=b; else preset=a; fi
fi

case "$preset" in
  a)
    config="(vec {
      record { principal \"$openhr\"; record {
        domains = opt vec { \"openhr.example.com\" };
        name = opt \"OpenHR\";
        icon = opt \"$(url "$openhr")/icon.svg\";
      } };
      record { principal \"$openteam\"; record {
        domains = opt vec { \"openteam.example.com\"; \"team.example.com\" };
        name = opt \"OpenTeam\";
        icon = opt \"$(url "$openteam")/icon.svg\";
      } };
      record { principal \"$launchpad\"; record {
        domains = opt vec { \"launchpad.example.com\" };
        name = opt \"Launchpad\";
        icon = null;
      } };
    })"
    ;;
  b)
    # openteam loses its domains and its icon here: an empty list, which
    # empties its two files, and no icon, which the launchpad draws a
    # placeholder for.
    config="(vec {
      record { principal \"$openhr\"; record {
        domains = opt vec { \"people.example.org\" };
        name = opt \"People & HR\";
        icon = opt \"$(url "$openhr")/icon-alt.svg\";
      } };
      record { principal \"$openteam\"; record {
        domains = opt vec {};
        name = opt \"Team Directory\";
        icon = null;
      } };
      record { principal \"$launchpad\"; record {
        domains = opt vec { \"apps.example.org\" };
        name = opt \"App Launcher\";
        icon = null;
      } };
    })"
    ;;
  *)
    echo "usage: $0 [a|b]" >&2
    exit 2
    ;;
esac

echo "Sending preset $preset to every canister:"
for canister in openhr openteam launchpad; do
  icp canister call --candid "$ASSETS_DID" "$canister" on_config_update "$config" >/dev/null
  echo "  $canister"
done
icp canister call --candid "$BACKEND_DID" launchpad-backend on_config_update "$config" >/dev/null
echo "  launchpad-backend"

echo
echo "Each asset canister wrote its own domains to /.well-known/ic-domains:"
for canister in openhr openteam launchpad; do
  id=$(icp canister status "$canister" --id-only)
  echo "  $canister"
  # Asked of the gateway by canister id, which needs no `*.localhost` lookup.
  curl -fsS "$gateway/.well-known/ic-domains?canisterId=$id" | sed 's/^/    /'
done

echo
echo "The launchpad shows the new names: $(url "$launchpad")/"
