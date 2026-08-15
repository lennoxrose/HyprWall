#!/usr/bin/env bash
# Temporary install path while hyprwall isn't on the AUR yet (registration
# has been closed since the June 2026 malware wave -- see README.md's
# "Future Plans" section). Runs the exact same recipe the eventual
# hyprwall-git AUR package uses (packaging/PKGBUILD), just fetched and built
# locally with makepkg instead of through AUR infrastructure.
#
#   curl -fsSL https://raw.githubusercontent.com/lennoxrose/HyprWall/master/install.sh | bash
set -euo pipefail

REPO_URL="https://github.com/lennoxrose/HyprWall.git"

if [ "$(id -u)" -eq 0 ]; then
  echo "error: don't run this as root -- it calls sudo itself where needed, and makepkg refuses to run as root anyway." >&2
  exit 1
fi

if ! command -v pacman >/dev/null 2>&1; then
  echo "error: this installer only supports Arch Linux and Arch-based distros (needs pacman + makepkg)." >&2
  echo "See README.md for manual build instructions on other distros." >&2
  exit 1
fi

echo "==> Installing build dependencies (base-devel, git)"
sudo pacman -S --needed --noconfirm base-devel git

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

echo "==> Cloning HyprWall"
git clone --depth 1 "$REPO_URL" "$WORKDIR/hyprwall"

echo "==> Building and installing via makepkg"
cd "$WORKDIR/hyprwall/packaging"
makepkg -si

echo "==> Done. See README.md for how to launch hyprwalld and hyprwall-gui."
