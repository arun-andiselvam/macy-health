#!/usr/bin/env bash
# Builds a universal (Apple Silicon + Intel) macOS release: signs, notarizes,
# staples, and signs the auto-update bundle. With --publish, uploads it to the
# GitHub release for the current version and publishes that release.
#
# One-time setup on the release machine:
#   - Developer ID cert in the login keychain (identity is in tauri.conf.json)
#   - notarytool profile:  xcrun notarytool store-credentials hitasoft-notary ...
#   - updater key at ~/.tauri/macy-health.key, its password in the keychain
#     (service "macy-health-updater-key")
#   - gh auth login (only for --publish)
set -euo pipefail

PUBLISH=false
[ "${1:-}" = "--publish" ] && PUBLISH=true

PROFILE="${NOTARY_PROFILE:-hitasoft-notary}"
REPO="arun-andiselvam/macy-health"
TARGET="universal-apple-darwin"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUNDLE="$ROOT/src-tauri/target/$TARGET/release/bundle"

cd "$ROOT"
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"
VERSION="$(node -p "require('./src-tauri/tauri.conf.json').version")"
TAG="v$VERSION"

export TAURI_SIGNING_PRIVATE_KEY="$(cat "$HOME/.tauri/macy-health.key")"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(security find-generic-password -s macy-health-updater-key -a macy-health -w)"

echo "==> Building Macy Health $VERSION ($TARGET)"
pnpm tauri build --target "$TARGET"

APP="$BUNDLE/macos/Macy Health.app"
DMG="$(ls -t "$BUNDLE"/dmg/*.dmg | head -1)"
codesign --verify --deep --strict "$APP"

echo "==> Notarizing $(basename "$DMG") (usually 1–5 minutes)"
RESULT="$BUNDLE/notarization.json"
xcrun notarytool submit "$DMG" --keychain-profile "$PROFILE" --wait --output-format json | tee "$RESULT"
echo
if ! grep -q '"status":"Accepted"' "$RESULT"; then
  id="$(sed -E 's/.*"id":"([^"]+)".*/\1/' "$RESULT")"
  echo "Notarization failed. Log:"
  xcrun notarytool log "$id" --keychain-profile "$PROFILE"
  exit 1
fi

echo "==> Stapling"
xcrun stapler staple "$DMG"
xcrun stapler staple "$APP"
spctl -a -t install "$DMG"
spctl -a -t exec "$APP"

# Rebuild the update bundle from the stapled app, and re-sign it.
OUT="$BUNDLE/release"
rm -rf "$OUT" && mkdir -p "$OUT"
DMG_OUT="$OUT/Macy-Health_${VERSION}_universal.dmg"
TAR_OUT="$OUT/Macy-Health_${VERSION}_universal.app.tar.gz"
cp "$DMG" "$DMG_OUT"
tar -C "$BUNDLE/macos" -czf "$TAR_OUT" "Macy Health.app"
pnpm -s tauri signer sign "$TAR_OUT" >/dev/null
echo "==> Built:"; ls -lh "$OUT"

$PUBLISH || { echo "Not publishing (pass --publish to upload to GitHub)."; exit 0; }

echo "==> Publishing to $REPO $TAG"
gh release view "$TAG" --repo "$REPO" >/dev/null 2>&1 \
  || gh release create "$TAG" --repo "$REPO" --draft --title "Macy Health $TAG" --notes "Download the installer for your system below."

# Merge macOS entries into latest.json (Windows/Linux entries come from CI).
LATEST="$OUT/latest.json"
gh release download "$TAG" --repo "$REPO" --pattern latest.json --dir "$OUT" --clobber 2>/dev/null || echo '{}' > "$LATEST"
URL="https://github.com/$REPO/releases/download/$TAG/$(basename "$TAR_OUT")"
python3 - "$LATEST" "$VERSION" "$URL" "$TAR_OUT.sig" <<'PY'
import json, sys, datetime
path, version, url, sig_path = sys.argv[1:]
data = json.load(open(path))
data.setdefault("version", version)
data.setdefault("notes", f"Macy Health {version}")
data.setdefault("pub_date", datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"))
entry = {"signature": open(sig_path).read().strip(), "url": url}
platforms = data.setdefault("platforms", {})
for key in ("darwin-aarch64", "darwin-x86_64", "darwin-aarch64-app", "darwin-x86_64-app"):
    platforms[key] = entry
json.dump(data, open(path, "w"), indent=2)
PY

gh release upload "$TAG" --repo "$REPO" --clobber "$DMG_OUT" "$TAR_OUT" "$TAR_OUT.sig" "$LATEST"
gh release edit "$TAG" --repo "$REPO" --draft=false --latest
echo "Published: https://github.com/$REPO/releases/tag/$TAG"
