#!/usr/bin/env bash
set -euo pipefail

repo="${MOI_GITHUB_REPOSITORY:-konattsu/moi}"
self_path="${MOI_SELF_PATH:-$HOME/.local/bin/moi}"

usage() {
  cat >&2 <<'EOF'
usage: curl -fsSL https://raw.githubusercontent.com/konattsu/moi/main/install.sh | bash -s -- [global options] plan|apply [options] [module ...]
EOF
}

target_triple() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux) ;;
    *)
      echo "error: unsupported os: $os" >&2
      exit 1
      ;;
  esac

  case "$arch" in
    x86_64|amd64) echo "x86_64-unknown-linux-gnu" ;;
    aarch64|arm64) echo "aarch64-unknown-linux-gnu" ;;
    *)
      echo "error: unsupported architecture: $arch" >&2
      exit 1
      ;;
  esac
}

if (($# == 0)); then
  usage
  exit 1
fi

command -v curl >/dev/null 2>&1 || {
  echo "error: missing required command: curl" >&2
  exit 1
}
command -v sha256sum >/dev/null 2>&1 || {
  echo "error: missing required command: sha256sum" >&2
  exit 1
}
command -v tar >/dev/null 2>&1 || {
  echo "error: missing required command: tar" >&2
  exit 1
}

target="$(target_triple)"
package="moi-$target"
asset="${MOI_RELEASE_ASSET:-$package.tar.gz}"
release_base="${MOI_RELEASE_BASE:-https://github.com/$repo/releases/latest/download}"
url="$release_base/$asset"

mkdir -p "$(dirname "$self_path")"
tmp="$(mktemp -d)"
replacement=""
trap 'rm -rf -- "$tmp"; if [[ -n "$replacement" ]]; then rm -f -- "$replacement"; fi' EXIT

curl -fsSL "$url" -o "$tmp/$asset"
curl -fsSL "$url.sha256" -o "$tmp/$asset.sha256"

(
  cd "$tmp"
  sha256sum --check --status "$asset.sha256"
  tar -xzf "$asset"
)

replacement="$(mktemp "$(dirname "$self_path")/.moi-install.XXXXXXXX")"
install -m 755 "$tmp/$package/moi" "$replacement"
mv -fT -- "$replacement" "$self_path"
replacement=""
rm -rf -- "$tmp"
trap - EXIT

exec "$self_path" "$@"
