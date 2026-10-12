#!/bin/sh
# Cymatrox Viewer — Linux installer
#
# Downloads the right package for this machine from the project's GitHub
# releases and installs it:
#
#   * Debian, Ubuntu and derivatives -> the .deb package, through apt
#   * any other distribution         -> the AppImage, in ~/.local/bin
#
# The script is self-contained: it needs no checkout of the repository, only
# curl (or wget) and the usual coreutils.
#
# Usage: install.sh [options]
#
#   -v, --version <tag>   release to install (default: the latest)
#   -m, --method <how>    force "deb" or "appimage"
#   -d, --dir <path>      where to put the AppImage (default: ~/.local/bin)
#   -u, --uninstall       remove an existing installation
#   -h, --help            show this help
#
# Examples:
#   install.sh                                  # choose automatically
#   install.sh --version v0.1.1
#   install.sh --method appimage --dir /opt/bin
#   install.sh --uninstall

set -eu

APP=cymatrox-viewer
REPO=YaogoGerard/cymatrox-viewer
PRODUCT="Cymatrox Viewer"
RELEASES="https://github.com/$REPO/releases"

: "${HOME:?HOME is not set}"

BIN_DIR=$HOME/.local/bin
DATA_DIR=${XDG_DATA_HOME:-$HOME/.local/share}
VERSION=
METHOD=
UNINSTALL=0
SUDO=

info() { printf '  %s\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

usage() {
	cat <<EOF
$PRODUCT — Linux installer

Usage: install.sh [options]

  -v, --version <tag>   release to install (default: the latest)
  -m, --method <how>    force "deb" or "appimage"
  -d, --dir <path>      where to put the AppImage (default: ~/.local/bin)
  -u, --uninstall       remove an existing installation
  -h, --help            show this help

Debian and Ubuntu get the .deb (installed with apt, so it asks for your
password); every other distribution gets the AppImage, which needs no root.

Released under the MIT license. Project: $RELEASES
EOF
}

# Print the contents of a URL (or fail). Quiet: used for the small files.
fetch() {
	if command -v curl >/dev/null 2>&1; then
		curl -fsSL --retry 3 "$1"
	elif command -v wget >/dev/null 2>&1; then
		wget -q --tries=3 -O - "$1"
	else
		die "curl or wget is required to download the package"
	fi
}

# Download the package itself. The file is tens of megabytes, so show a progress
# bar when a terminal is attached — and stay quiet when the output is piped.
download_file() { # <url> <destination>
	if command -v curl >/dev/null 2>&1; then
		if [ -t 2 ]; then
			curl -fL --retry 3 --progress-bar -o "$2" "$1"
		else
			curl -fsSL --retry 3 -o "$2" "$1"
		fi
	elif command -v wget >/dev/null 2>&1; then
		if [ -t 2 ]; then
			wget -O "$2" "$1"
		else
			wget -q -O "$2" "$1"
		fi
	else
		die "curl or wget is required to download the package"
	fi
}

# Run a command as root, using sudo only when we are not root already.
run_root() {
	if [ -n "$SUDO" ]; then
		"$SUDO" "$@"
	else
		"$@"
	fi
}

require_linux_x86_64() {
	[ "$(uname -s)" = Linux ] ||
		die "this installer is for Linux only; get the build for your system from $RELEASES/latest"

	MACHINE=$(uname -m)
	case "$MACHINE" in
	x86_64 | amd64) ;;
	*) die "no Linux build is published for '$MACHINE' (only x86_64). See $RELEASES" ;;
	esac
}

detect_sudo() {
	if [ "$(id -u)" -eq 0 ]; then
		SUDO=
	elif command -v sudo >/dev/null 2>&1; then
		SUDO=sudo
	fi
}

resolve_version() {
	[ -n "$VERSION" ] && return

	info "looking up the latest release..."
	json=$TMP/api.json
	fetch "https://api.github.com/repos/$REPO/releases/latest" >"$json" ||
		die "could not reach GitHub; check your connection or pass a version with --version"
	VERSION=$(sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$json" | head -n 1)
	[ -n "$VERSION" ] || die "could not read the latest version; pass one with --version"
}

choose_method() {
	if [ -z "$METHOD" ]; then
		if command -v dpkg >/dev/null 2>&1 && command -v apt-get >/dev/null 2>&1; then
			METHOD=deb
		else
			METHOD=appimage
		fi
	fi

	case "$METHOD" in
	deb)
		if ! command -v apt-get >/dev/null 2>&1; then
			die "no apt-get here; install the AppImage instead with --method appimage"
		fi
		if [ "$(id -u)" -ne 0 ] && [ -z "$SUDO" ]; then
			warn "installing a .deb needs root and sudo is unavailable — using the AppImage instead"
			METHOD=appimage
		fi
		;;
	appimage) ;;
	*) die "unknown method '$METHOD' (expected \"deb\" or \"appimage\")" ;;
	esac
}

install_deb() {
	info "installing with apt — you may be asked for your password"
	if run_root apt-get install -y "$1"; then
		return
	fi
	warn "apt refused the package; retrying with dpkg"
	run_root dpkg -i "$1" || true
	run_root apt-get -f install -y
}

install_appimage() {
	mkdir -p "$BIN_DIR"
	cp "$1" "$BIN_DIR/$APP"
	chmod +x "$BIN_DIR/$APP"
	info "installed $BIN_DIR/$APP"

	# Menu entry and icon, in the user's own XDG directories (no root).
	entry_dir=$DATA_DIR/applications
	icon_dir=$DATA_DIR/icons/hicolor/512x512/apps
	mkdir -p "$entry_dir" "$icon_dir"

	fetch "https://raw.githubusercontent.com/$REPO/$VERSION/build/icons/icon.png" >"$icon_dir/$APP.png" ||
		warn "could not download the icon — the menu entry will show a generic one"

	cat >"$entry_dir/$APP.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=$PRODUCT
Comment=Interactive cymatics laboratory
Exec=$BIN_DIR/$APP
Icon=$APP
Terminal=false
Categories=Science;Graphics;
EOF
	info "added the menu entry $entry_dir/$APP.desktop"

	case ":$PATH:" in
	*":$BIN_DIR:"*) ;;
	*)
		warn "$BIN_DIR is not in your PATH"
		info "add this to your shell profile:  export PATH=\"$BIN_DIR:\$PATH\""
		;;
	esac
}

uninstall() {
	found=0

	if [ -f "$BIN_DIR/$APP" ]; then
		rm -f "$BIN_DIR/$APP"
		info "removed $BIN_DIR/$APP"
		found=1
	fi
	if [ -f "$DATA_DIR/applications/$APP.desktop" ]; then
		rm -f "$DATA_DIR/applications/$APP.desktop"
		info "removed the menu entry"
		found=1
	fi
	if [ -f "$DATA_DIR/icons/hicolor/512x512/apps/$APP.png" ]; then
		rm -f "$DATA_DIR/icons/hicolor/512x512/apps/$APP.png"
		info "removed the icon"
		found=1
	fi

	if command -v dpkg >/dev/null 2>&1 && dpkg -s "$APP" >/dev/null 2>&1; then
		info "removing the .deb package"
		run_root apt-get remove -y "$APP" || true
		found=1
	fi

	[ "$found" -eq 1 ] || info "nothing to remove"
	info "done"
}

main() {
	while [ $# -gt 0 ]; do
		case "$1" in
		-v | --version)
			[ $# -ge 2 ] || die "--version needs a value"
			VERSION=$2
			shift 2
			;;
		-m | --method)
			[ $# -ge 2 ] || die "--method needs a value"
			METHOD=$2
			shift 2
			;;
		-d | --dir)
			[ $# -ge 2 ] || die "--dir needs a value"
			BIN_DIR=$2
			shift 2
			;;
		-u | --uninstall)
			UNINSTALL=1
			shift
			;;
		-h | --help)
			usage
			exit 0
			;;
		*)
			die "unknown option: $1 (try --help)"
			;;
		esac
	done

	require_linux_x86_64
	detect_sudo

	if [ "$UNINSTALL" -eq 1 ]; then
		uninstall
		exit 0
	fi

	printf '%s\n' "$PRODUCT installer"

	TMP=$(mktemp -d "${TMPDIR:-/tmp}/cymatrox-viewer.XXXXXX")
	trap 'rm -rf "$TMP"' EXIT INT TERM

	resolve_version
	# Accept both "0.1.1" and "v0.1.1"; the release tag always carries the v.
	case "$VERSION" in
	v*) ;;
	*) VERSION="v$VERSION" ;;
	esac
	NUM=${VERSION#v}

	choose_method

	info "release : $VERSION"
	info "method  : $METHOD"

	case "$METHOD" in
	deb)
		name=${APP}_${NUM}_amd64.deb
		file=$TMP/$name
		info "downloading $name..."
		download_file "$RELEASES/download/$VERSION/$name" "$file" ||
			die "could not download $name for $VERSION"
		install_deb "$file"
		;;
	appimage)
		name=${APP}_${NUM}_x86_64.AppImage
		file=$TMP/$name
		info "downloading $name..."
		download_file "$RELEASES/download/$VERSION/$name" "$file" ||
			die "could not download $name for $VERSION"
		chmod +x "$file"
		install_appimage "$file"
		;;
	esac

	printf '\n'
	info "done — start $PRODUCT from your applications menu, or run:"
	info "  $([ "$METHOD" = deb ] && echo "$APP" || echo "$BIN_DIR/$APP")"
}

main "$@"
