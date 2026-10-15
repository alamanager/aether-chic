#!/usr/bin/env bash
# Downloads pinned sidecar engines (sing-box + Xray) for manual configs
# flat into src-tauri/binaries/ (covered by the tauri resources glob).
# Cross-build override like fetch-aether.sh: SINGBOX_ASSET / XRAY_ASSET.
set -euo pipefail

SINGBOX_VERSION="1.14.3"
XRAY_VERSION="v26.3.27"
DEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)   SB="sing-box-1.14.3-linux-amd64.tar.gz"; XR="Xray-linux-64.zip" ;;
  Darwin-arm64)   SB="sing-box-1.14.3-darwin-arm64.tar.gz"; XR="Xray-macos-arm64-v8a.zip" ;;
  Darwin-x86_64)  SB="sing-box-1.14.3-darwin-amd64.tar.gz"; XR="Xray-macos-64.zip" ;;
  *) echo "Unsupported platform for sidecars: $(uname -s)-$(uname -m) (Windows handled in CI)" >&2; exit 1 ;;
esac

SB="${SINGBOX_ASSET:-$SB}"
XR="${XRAY_ASSET:-$XR}"

cd "$DEST_DIR"
rm -rf sidecar-tmp && mkdir sidecar-tmp && cd sidecar-tmp
# -f: fail on HTTP errors so a 404 page never poses as a binary.
curl -sSLf -o sb.tar.gz "https://github.com/SagerNet/sing-box/releases/download/v${SINGBOX_VERSION}/$SB"
curl -sSLf -o xray.zip "https://github.com/XTLS/Xray-core/releases/download/${XRAY_VERSION}/$XR"
# Xray publishes .dgst sidecars: verify when present (python3 exists on
# every runner; sha256sum does not exist on macOS).
curl -sSLf -o xray.dgst "https://github.com/XTLS/Xray-core/releases/download/${XRAY_VERSION}/$XR.dgst"
want="$(grep -Eo '[0-9a-fA-F]{64}' xray.dgst | head -1)"
got="$(python3 -c 'import hashlib,sys; print(hashlib.sha256(open("xray.zip","rb").read()).hexdigest())')"
if [ "$want" != "$got" ]; then
  echo "Checksum verification failed for $XR" >&2
  exit 1
fi
tar xzf sb.tar.gz
unzip -o -q xray.zip
SBBIN="$(find . -name 'sing-box' -type f | head -1)"
test -n "$SBBIN"
mv "$SBBIN" ../sing-box
chmod +x ../sing-box
# Xray zips ship geoip.dat + geosite.dat at root: keep them beside xray.
mv xray ../xray 2>/dev/null || mv ./xray ../xray
chmod +x ../xray
for geo in geoip.dat geosite.dat; do
  [ -f "$geo" ] && mv "$geo" "../$geo"
done
cd "$DEST_DIR" && rm -rf sidecar-tmp
echo "Sidecars ready at $DEST_DIR/sing-box and $DEST_DIR/xray"
