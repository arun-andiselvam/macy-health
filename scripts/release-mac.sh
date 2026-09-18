#!/usr/bin/env bash
# Builds, signs, notarizes and staples the macOS app + dmg.
#
# Signing uses the Developer ID identity in tauri.conf.json (login keychain).
# Notarization uses a notarytool keychain profile, so no secrets live in the
# repo. Create it once with:
#   xcrun notarytool store-credentials hitasoft-notary --key <p8> --key-id <id> --issuer <uuid>
set -euo pipefail

PROFILE="${NOTARY_PROFILE:-hitasoft-notary}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUNDLE="$ROOT/src-tauri/target/release/bundle"

cd "$ROOT"
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"

echo "==> Building signed app and dmg"
pnpm tauri build

APP="$(ls -d "$BUNDLE"/macos/*.app | head -1)"
DMG="$(ls -t "$BUNDLE"/dmg/*.dmg | head -1)"

echo "==> Verifying signature"
codesign --verify --deep --strict "$APP"

echo "==> Notarizing $(basename "$DMG") (usually 1–5 minutes)"
xcrun notarytool submit "$DMG" --keychain-profile "$PROFILE" --wait --output-format json \
  | tee "$BUNDLE/notarization.json"
grep -q '"status":"Accepted"' "$BUNDLE/notarization.json" || {
  id="$(sed -E 's/.*"id":"([^"]+)".*/\1/' "$BUNDLE/notarization.json")"
  echo "Notarization failed. Log:"; xcrun notarytool log "$id" --keychain-profile "$PROFILE"
  exit 1
}

echo "==> Stapling tickets"
xcrun stapler staple "$DMG"
xcrun stapler staple "$APP"

echo "==> Gatekeeper check"
spctl -a -vvv -t install "$DMG"
spctl -a -vvv -t exec "$APP"

echo "Done: $DMG"
