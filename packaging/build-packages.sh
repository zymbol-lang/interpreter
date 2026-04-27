#!/usr/bin/env bash
# build-packages.sh — Zymbol packaging script
# Produces .deb, .rpm, .pkg.tar.zst, and a static musl binary from the compiled source.
#
# Usage: bash packaging/build-packages.sh [OPTIONS]
# Run from interpreter/ (the git repository root).

set -euo pipefail

# ---------------------------------------------------------------------------
# Defaults
# ---------------------------------------------------------------------------
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMPLATES_DIR="${REPO_ROOT}/packaging/templates"
OUT_DIR="${REPO_ROOT}/packaging/dist"
FORMATS="all"
ARCH=""           # auto-detected below
VERSION=""        # auto-detected below
DO_BUILD=true
USE_CROSS=false
DO_HASHES=true
USE_TIMESTAMP=true

# ---------------------------------------------------------------------------
# Colours
# ---------------------------------------------------------------------------
RED='\033[0;31m'; YELLOW='\033[1;33m'; GREEN='\033[0;32m'; CYAN='\033[0;36m'; NC='\033[0m'
info()    { echo -e "${CYAN}[INFO]${NC}  $*"; }
success() { echo -e "${GREEN}[OK]${NC}    $*"; }
warn()    { echo -e "${YELLOW}[WARN]${NC}  $*"; }
error()   { echo -e "${RED}[ERROR]${NC} $*" >&2; exit 1; }

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------
usage() {
    cat <<EOF
Usage: bash packaging/build-packages.sh [OPTIONS]

Options:
  --version  X.Y.Z       Override version (default: read from interpreter/Cargo.toml)
  --arch     ARCH        x86_64 | aarch64  (default: host arch via uname -m)
  --formats  LIST        Comma-separated subset: deb,rpm,arch,static,win,winmsi  (default: all)
                         Note: win/winmsi are for local testing only — release Windows builds
                         run on GitHub Actions (release-windows.yml) using MSVC.
  --cross                Cross-compile with 'cross' (required for aarch64 on x86_64 host)
  --no-build             Skip cargo/cross build; use existing binary
  --no-hashes            Skip SHA256SUMS / SHA512SUMS generation
  --no-timestamp         Use canonical names without timestamp (for release builds)
  --out-dir  PATH        Output directory (default: packaging/dist/)
  -h, --help             Show this help

Examples:
  bash packaging/build-packages.sh
  bash packaging/build-packages.sh --formats deb,static
  bash packaging/build-packages.sh --arch aarch64 --cross
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version)  VERSION="$2";  shift 2 ;;
        --arch)     ARCH="$2";     shift 2 ;;
        --formats)  FORMATS="$2";  shift 2 ;;
        --cross)        USE_CROSS=true;    shift ;;
        --no-build)     DO_BUILD=false;    shift ;;
        --no-hashes)    DO_HASHES=false;   shift ;;
        --no-timestamp) USE_TIMESTAMP=false; shift ;;
        --out-dir)      OUT_DIR="$2";      shift 2 ;;
        -h|--help)  usage; exit 0 ;;
        *) error "Unknown option: $1" ;;
    esac
done

# ---------------------------------------------------------------------------
# Detect version
# ---------------------------------------------------------------------------
if [[ -z "${VERSION}" ]]; then
    CARGO_TOML="${REPO_ROOT}/Cargo.toml"
    [[ -f "${CARGO_TOML}" ]] || error "Cargo.toml not found — run from interpreter/"
    VERSION=$(grep '^version' "${CARGO_TOML}" | head -1 | cut -d'"' -f2)
    [[ -n "${VERSION}" ]] || error "Could not parse version from Cargo.toml"
fi
info "Version: ${VERSION}"

# ---------------------------------------------------------------------------
# Detect / validate architecture
# ---------------------------------------------------------------------------
if [[ -z "${ARCH}" ]]; then
    HOST_ARCH="$(uname -m)"
    case "${HOST_ARCH}" in
        x86_64)  ARCH="x86_64" ;;
        aarch64) ARCH="aarch64" ;;
        *) error "Unsupported host architecture: ${HOST_ARCH}. Use --arch x86_64|aarch64" ;;
    esac
fi
[[ "${ARCH}" == "x86_64" || "${ARCH}" == "aarch64" ]] \
    || error "Invalid --arch '${ARCH}'. Must be x86_64 or aarch64."
info "Architecture: ${ARCH}"

# Mapping tables
case "${ARCH}" in
    x86_64)
        DEB_ARCH="amd64"
        RPM_ARCH="x86_64"
        RUST_TARGET="x86_64-unknown-linux-gnu"
        APPIMAGE_ARCH="x86_64"
        ;;
    aarch64)
        DEB_ARCH="arm64"
        RPM_ARCH="aarch64"
        RUST_TARGET="aarch64-unknown-linux-gnu"
        APPIMAGE_ARCH="aarch64"
        ;;
esac

# ---------------------------------------------------------------------------
# Formats to build
# ---------------------------------------------------------------------------
build_deb=false; build_rpm=false; build_arch=false; build_static=false
build_win=false; build_winmsi=false
IFS=',' read -ra fmt_list <<< "${FORMATS}"
for f in "${fmt_list[@]}"; do
    case "${f}" in
        deb)      build_deb=true ;;
        rpm)      build_rpm=true ;;
        arch)     build_arch=true ;;
        static)   build_static=true ;;
        win)      build_win=true ;;
        winmsi)   build_winmsi=true ;;
        all)      build_deb=true; build_rpm=true; build_arch=true
                  build_static=true; build_win=true; build_winmsi=true ;;
        *) error "Unknown format '${f}'. Valid: deb,rpm,arch,static,win,winmsi,all" ;;
    esac
done

# ---------------------------------------------------------------------------
# Timestamp and base name
# ---------------------------------------------------------------------------
TIMESTAMP="$(date -u +%Y%m%dT%H%M)"
if [[ "${USE_TIMESTAMP}" == true ]]; then
    BASE_NAME="zymbol_lang_v${VERSION}_${ARCH}_${TIMESTAMP}"
else
    BASE_NAME="zymbol_lang_v${VERSION}_${ARCH}"
fi
info "Package base name: ${BASE_NAME}"

# ---------------------------------------------------------------------------
# Build binary
# ---------------------------------------------------------------------------
INTERP_DIR="${REPO_ROOT}"

if [[ "${DO_BUILD}" == true ]]; then
    info "Building binary for ${RUST_TARGET}..."
    cd "${INTERP_DIR}"

    HOST_ARCH="$(uname -m)"
    NEED_CROSS=false
    if [[ "${ARCH}" == "aarch64" && "${HOST_ARCH}" == "x86_64" ]]; then
        NEED_CROSS=true
    elif [[ "${ARCH}" == "x86_64" && "${HOST_ARCH}" == "aarch64" ]]; then
        NEED_CROSS=true
    fi

    if [[ "${NEED_CROSS}" == true ]]; then
        # Prefer native cross-linker (aarch64-linux-gnu-gcc) over 'cross' (Docker)
        if command -v "aarch64-linux-gnu-gcc" &>/dev/null; then
            info "  Cross-compiling with cargo + aarch64-linux-gnu-gcc"
            cargo build --release --target "${RUST_TARGET}"
            BINARY="${INTERP_DIR}/target/${RUST_TARGET}/release/zymbol"
        elif [[ "${USE_CROSS}" == true ]]; then
            command -v cross &>/dev/null \
                || error "'cross' not found in PATH. Install with: cargo install cross"
            info "  Cross-compiling with 'cross' (Docker)"
            cross build --release --target "${RUST_TARGET}"
            BINARY="${INTERP_DIR}/target/${RUST_TARGET}/release/zymbol"
        else
            error "Cross-compilation needed (host: ${HOST_ARCH}, target: ${ARCH}).
Options:
  1) Install the native linker: sudo apt install gcc-aarch64-linux-gnu
     Then add to interpreter/.cargo/config.toml:
       [target.aarch64-unknown-linux-gnu]
       linker = \"aarch64-linux-gnu-gcc\"
  2) Use 'cross' (Docker): cargo install cross  →  pass --cross"
        fi
    else
        cargo build --release
        BINARY="${INTERP_DIR}/target/release/zymbol"
    fi
    cd "${REPO_ROOT}"
else
    info "Skipping build (--no-build). Looking for existing binary..."
    # Prefer arch-specific path, fall back to default release path
    if [[ -f "${INTERP_DIR}/target/${RUST_TARGET}/release/zymbol" ]]; then
        BINARY="${INTERP_DIR}/target/${RUST_TARGET}/release/zymbol"
    elif [[ -f "${INTERP_DIR}/target/release/zymbol" ]]; then
        BINARY="${INTERP_DIR}/target/release/zymbol"
    else
        error "No binary found. Run without --no-build or build manually first."
    fi
fi

[[ -f "${BINARY}" ]] || error "Binary not found at: ${BINARY}"
success "Binary ready: ${BINARY}"

# ---------------------------------------------------------------------------
# Shared assets
# ---------------------------------------------------------------------------
ICON="${REPO_ROOT}/logo.png"
[[ -f "${ICON}" ]] || { warn "logo.png not found — icon will be absent in packages"; ICON=""; }

COPYRIGHT_SRC="${REPO_ROOT}/LICENSE"
[[ -f "${COPYRIGHT_SRC}" ]] || COPYRIGHT_SRC="${REPO_ROOT}/LICENSE-AGPL-3.0"
[[ -f "${COPYRIGHT_SRC}" ]] || { warn "LICENSE file not found — copyright will be empty"; COPYRIGHT_SRC=""; }

DESKTOP_SRC="${TEMPLATES_DIR}/zymbol.desktop"

# ---------------------------------------------------------------------------
# Output directory
# ---------------------------------------------------------------------------
mkdir -p "${OUT_DIR}"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
make_source_tarball() {
    # make_source_tarball <dir_name> <tarball_path>
    # Creates a source tarball from a staging directory (for rpm/arch).
    local dir="$1" out="$2"
    tar -C "$(dirname "${dir}")" -czf "${out}" "$(basename "${dir}")"
}

# ---------------------------------------------------------------------------
# .deb
# ---------------------------------------------------------------------------
build_deb_package() {
    info "Building .deb..."
    local tmp; tmp="$(mktemp -d)"
    trap 'rm -rf "${tmp}"' RETURN

    local staging="${tmp}/staging"
    mkdir -p "${staging}/DEBIAN"
    mkdir -p "${staging}/usr/bin"
    mkdir -p "${staging}/usr/share/doc/zymbol-lang"
    mkdir -p "${staging}/usr/share/applications"
    [[ -n "${ICON}" ]] && mkdir -p "${staging}/usr/share/pixmaps"

    # Control file
    sed \
        -e "s/{{VERSION}}/${VERSION}/g" \
        -e "s/{{DEB_ARCH}}/${DEB_ARCH}/g" \
        "${TEMPLATES_DIR}/control.in" > "${staging}/DEBIAN/control"

    # Binary
    install -m 755 "${BINARY}" "${staging}/usr/bin/zymbol"

    # Copyright
    if [[ -n "${COPYRIGHT_SRC}" ]]; then
        install -m 644 "${COPYRIGHT_SRC}" "${staging}/usr/share/doc/zymbol-lang/copyright"
    fi

    # Desktop entry
    install -m 644 "${DESKTOP_SRC}" "${staging}/usr/share/applications/zymbol.desktop"

    # Icon
    if [[ -n "${ICON}" ]]; then
        install -m 644 "${ICON}" "${staging}/usr/share/pixmaps/zymbol.png"
    fi

    local out="${OUT_DIR}/${BASE_NAME}.deb"
    dpkg-deb --build --root-owner-group "${staging}" "${out}"
    success ".deb → ${out}"
}

# ---------------------------------------------------------------------------
# .rpm
# ---------------------------------------------------------------------------
build_rpm_package() {
    info "Building .rpm..."
    command -v rpmbuild &>/dev/null \
        || error "'rpmbuild' not found. Install with:
  Fedora/RHEL:  sudo dnf install rpm-build
  openSUSE:     sudo zypper install rpm-build"

    local tmp; tmp="$(mktemp -d)"
    trap 'rm -rf "${tmp}"' RETURN

    # RPM build tree
    for d in BUILD RPMS SOURCES SPECS SRPMS; do mkdir -p "${tmp}/${d}"; done

    # Source dir for tarball
    local src_dir="${tmp}/src/zymbol-${VERSION}-${RPM_ARCH}"
    mkdir -p "${src_dir}"
    install -m 755 "${BINARY}" "${src_dir}/zymbol"
    install -m 644 "${DESKTOP_SRC}" "${src_dir}/zymbol.desktop"
    [[ -n "${ICON}" ]]           && install -m 644 "${ICON}" "${src_dir}/zymbol.png"
    [[ -n "${COPYRIGHT_SRC}" ]]  && install -m 644 "${COPYRIGHT_SRC}" "${src_dir}/copyright"

    local tarball="${tmp}/SOURCES/zymbol-${VERSION}-${RPM_ARCH}.tar.gz"
    make_source_tarball "${src_dir}" "${tarball}"

    # RPM date (Day Mon DD YYYY)
    local rpm_date; rpm_date="$(date -u +'%a %b %d %Y')"

    # Spec file
    sed \
        -e "s/{{VERSION}}/${VERSION}/g" \
        -e "s/{{RPM_ARCH}}/${RPM_ARCH}/g" \
        -e "s/{{RPM_DATE}}/${rpm_date}/g" \
        "${TEMPLATES_DIR}/zymbol.spec.in" > "${tmp}/SPECS/zymbol.spec"

    rpmbuild \
        --define "_topdir ${tmp}" \
        --define "_rpmdir ${tmp}/RPMS" \
        --define "_build_cpu ${RPM_ARCH}" \
        --define "_host_cpu ${RPM_ARCH}" \
        --define "_target_cpu ${RPM_ARCH}" \
        --define "optflags -O2" \
        --target "${RPM_ARCH}-linux-gnu" \
        -bb "${tmp}/SPECS/zymbol.spec"

    local built_rpm; built_rpm="$(find "${tmp}/RPMS" -name '*.rpm' | head -1)"
    [[ -n "${built_rpm}" ]] || error "rpmbuild did not produce a .rpm file"

    local out="${OUT_DIR}/${BASE_NAME}.rpm"
    mv "${built_rpm}" "${out}"
    success ".rpm → ${out}"
}

# ---------------------------------------------------------------------------
# .pkg.tar.zst (Arch) — built directly without makepkg or Docker
# ---------------------------------------------------------------------------
build_arch_package() {
    info "Building .pkg.tar.zst (Arch)..."

    command -v zstd &>/dev/null \
        || error "'zstd' not found. Install with: sudo apt install zstd"

    local tmp; tmp="$(mktemp -d)"
    trap 'rm -rf "${tmp}"' RETURN

    local pkg_dir="${tmp}/pkg"

    # Install tree inside pkg_dir (mirrors the final filesystem layout)
    install -Dm755 "${BINARY}"      "${pkg_dir}/usr/bin/zymbol"
    install -Dm644 "${DESKTOP_SRC}" "${pkg_dir}/usr/share/applications/zymbol.desktop"
    [[ -n "${ICON}" ]]          && install -Dm644 "${ICON}"          "${pkg_dir}/usr/share/pixmaps/zymbol.png"
    [[ -n "${COPYRIGHT_SRC}" ]] && install -Dm644 "${COPYRIGHT_SRC}" "${pkg_dir}/usr/share/doc/zymbol-lang/copyright"

    # Compute installed size in bytes (required by .PKGINFO)
    local installed_size; installed_size="$(du -sb "${pkg_dir}" | cut -f1)"

    # Write .PKGINFO — mandatory metadata file read by pacman
    cat > "${pkg_dir}/.PKGINFO" <<EOF
pkgname = zymbol-lang
pkgver = ${VERSION}-1
pkgdesc = Zymbol symbolic programming language — keyword-free, symbol-driven
url = https://zymbol-lang.org
builddate = $(date -u +%s)
packager = Zymbol-Lang Contributors
size = ${installed_size}
arch = ${ARCH}
license = AGPL-3.0-only
depend = glibc
EOF

    # Write .MTREE — file manifest (optional but expected by modern pacman)
    local mtree_raw="${tmp}/mtree_raw"
    (cd "${pkg_dir}" && find . \( -name '.PKGINFO' -o -name '.MTREE' \) -prune \
        -o -print | sort > "${mtree_raw}")

    local out="${OUT_DIR}/${BASE_NAME}.pkg.tar.zst"

    # Pack: .PKGINFO metadata + usr/ filesystem tree
    (cd "${pkg_dir}" && tar \
        --numeric-owner --owner=0 --group=0 \
        -c --zstd \
        -f "${out}" \
        .PKGINFO usr/ \
    )

    success ".pkg.tar.zst → ${out}"
}

# ---------------------------------------------------------------------------
# Static musl binary (no dependencies — works on any Linux)
# ---------------------------------------------------------------------------
build_static_package() {
    info "Building static musl binary..."

    # musl cross-compilation matrix
    local musl_target
    case "${ARCH}" in
        x86_64)  musl_target="x86_64-unknown-linux-musl" ;;
        aarch64) musl_target="aarch64-unknown-linux-musl" ;;
        *) error "No musl target mapping for arch: ${ARCH}" ;;
    esac

    # Ensure the Rust target is installed
    rustup target add "${musl_target}" &>/dev/null || true

    local out="${OUT_DIR}/${BASE_NAME}_linux"

    if [[ "${DO_BUILD}" == true ]]; then
        cd "${INTERP_DIR}"

        local linker_env=""
        if [[ "${ARCH}" == "aarch64" ]]; then
            command -v aarch64-linux-musl-gcc &>/dev/null \
                || error "aarch64 musl cross-compiler not found.
Install with: sudo apt install musl-tools gcc-aarch64-linux-gnu
And set up musl cross toolchain (e.g. musl-cross-make)."
            linker_env="CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-musl-gcc"
        fi

        env ${linker_env} RUSTFLAGS="-C target-feature=+crt-static" \
            cargo build --release --target "${musl_target}"

        cd "${REPO_ROOT}"
        local built="${INTERP_DIR}/target/${musl_target}/release/zymbol"
        [[ -f "${built}" ]] || error "Static binary not found at: ${built}"
        install -m 755 "${built}" "${out}"
    else
        [[ -f "${out}" ]] || error "Static binary not found at ${out} (and --no-build was set)"
    fi

    success "static binary → ${out}"
}

# ---------------------------------------------------------------------------
# Windows installer (.exe via NSIS)
# ---------------------------------------------------------------------------
build_windows_package() {
    info "Building Windows installer (.exe)..."

    command -v makensis &>/dev/null \
        || error "'makensis' not found. Install with: sudo apt install nsis"

    # Verify the Windows target is available
    rustup target list --installed 2>/dev/null | grep -q "x86_64-pc-windows-gnu" \
        || error "Rust target 'x86_64-pc-windows-gnu' not installed.
Run: rustup target add x86_64-pc-windows-gnu"

    command -v x86_64-w64-mingw32-gcc &>/dev/null \
        || error "MinGW cross-compiler not found. Install with: sudo apt install gcc-mingw-w64-x86-64"

    local tmp; tmp="$(mktemp -d)"
    trap 'rm -rf "${tmp}"' RETURN

    # Build Windows binary
    local win_binary="${INTERP_DIR}/target/x86_64-pc-windows-gnu/release/zymbol.exe"
    if [[ "${DO_BUILD}" == true ]]; then
        info "  Compiling zymbol.exe for Windows..."
        cd "${INTERP_DIR}"
        CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER="x86_64-w64-mingw32-gcc" \
            cargo build --release --target x86_64-pc-windows-gnu
        cd "${REPO_ROOT}"
    fi

    [[ -f "${win_binary}" ]] \
        || error "Windows binary not found at: ${win_binary}"

    # Prepare staging directory for NSIS
    local staging="${tmp}/nsis"
    mkdir -p "${staging}"

    install -m 644 "${win_binary}" "${staging}/zymbol.exe"
    [[ -n "${COPYRIGHT_SRC}" ]] && install -m 644 "${COPYRIGHT_SRC}" "${staging}/LICENSE"

    # Convert logo.png to proper multi-size .ico
    # Requires: icotool (sudo apt install icoutils) + ffmpeg (sudo apt install ffmpeg)
    local has_ico=false
    local ico="${staging}/logo.ico"
    if [[ -n "${ICON}" ]] && command -v ffmpeg &>/dev/null && command -v icotool &>/dev/null; then
        local tmp_ico_dir; tmp_ico_dir="$(mktemp -d)"
        for size in 16 32 48 256; do
            ffmpeg -y -i "${ICON}" -vf "scale=${size}:${size}" \
                "${tmp_ico_dir}/icon_${size}.png" &>/dev/null
        done
        icotool -c -o "${ico}" \
            "${tmp_ico_dir}/icon_16.png" \
            "${tmp_ico_dir}/icon_32.png" \
            "${tmp_ico_dir}/icon_48.png" \
            "${tmp_ico_dir}/icon_256.png" 2>/dev/null && has_ico=true
        rm -rf "${tmp_ico_dir}"
    fi

    # Generate the .nsi from template — strip icon directives if no valid .ico
    local nsi="${staging}/zymbol-setup.nsi"
    local win_filesuffix="${TIMESTAMP}"
    [[ "${USE_TIMESTAMP}" == false ]] && win_filesuffix="windows"
    sed \
        -e "s/{{VERSION}}/${VERSION}/g" \
        -e "s/{{TIMESTAMP}}/${win_filesuffix}/g" \
        "${TEMPLATES_DIR}/zymbol-setup.nsi.in" > "${nsi}"

    # Remove MUI_ICON / MUI_UNICON lines if no valid icon was produced
    if [[ "${has_ico}" == false ]]; then
        sed -i '/MUI_ICON\|MUI_UNICON/d' "${nsi}"
        warn "No valid .ico produced — installer will use default NSIS icon"
    fi

    # Run makensis from the staging dir so File paths resolve correctly
    local out_name="zymbol_lang_v${VERSION}_x86_64_${win_filesuffix}_setup.exe"
    local final_name; [[ "${USE_TIMESTAMP}" == false ]] && final_name="zymbol_lang_v${VERSION}_x86_64_windows.exe" || final_name="${out_name}"
    (cd "${staging}" && makensis -V2 "${nsi}")

    local built="${staging}/${out_name}"
    [[ -f "${built}" ]] || error "makensis did not produce ${out_name}"

    mv "${built}" "${OUT_DIR}/${final_name}"
    success ".exe → ${OUT_DIR}/${final_name}"
}

# ---------------------------------------------------------------------------
# Windows MSI package (via wixl — Linux WiX implementation)
# ---------------------------------------------------------------------------
build_windows_msi_package() {
    info "Building Windows MSI package..."

    command -v wixl &>/dev/null \
        || error "'wixl' not found. Install with: sudo apt install wixl"

    # Reuse the Windows binary built by build_windows_package or cargo
    local win_binary="${INTERP_DIR}/target/x86_64-pc-windows-gnu/release/zymbol.exe"
    if [[ "${DO_BUILD}" == true ]] && [[ ! -f "${win_binary}" ]]; then
        info "  Compiling zymbol.exe for Windows..."
        cd "${INTERP_DIR}"
        CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER="x86_64-w64-mingw32-gcc" \
            cargo build --release --target x86_64-pc-windows-gnu
        cd "${REPO_ROOT}"
    fi

    [[ -f "${win_binary}" ]] \
        || error "Windows binary not found at: ${win_binary}
Build it first with --formats win, or run without --no-build."

    local tmp; tmp="$(mktemp -d)"
    trap 'rm -rf "${tmp}"' RETURN

    local staging="${tmp}/msi"
    mkdir -p "${staging}"

    install -m 644 "${win_binary}" "${staging}/zymbol.exe"
    [[ -n "${COPYRIGHT_SRC}" ]] && install -m 644 "${COPYRIGHT_SRC}" "${staging}/LICENSE"

    # Stable upgrade GUID derived from package name (fixed per product, never changes)
    local guid_upgrade="A1B2C3D4-E5F6-7890-ABCD-EF1234567890"

    # Generate .wxs from template
    local wxs="${staging}/zymbol.wxs"
    sed \
        -e "s/{{VERSION}}/${VERSION}/g" \
        -e "s/{{GUID_UPGRADE}}/${guid_upgrade}/g" \
        "${TEMPLATES_DIR}/zymbol.wxs.in" > "${wxs}"

    local msi_suffix="${TIMESTAMP}"
    [[ "${USE_TIMESTAMP}" == false ]] && msi_suffix="windows"
    local out_name="zymbol_lang_v${VERSION}_x86_64_${msi_suffix}.msi"
    local out="${OUT_DIR}/${out_name}"

    (cd "${staging}" && wixl -a x64 -o "${out}" "${wxs}")

    [[ -f "${out}" ]] || error "wixl did not produce the .msi file"
    success ".msi → ${out}"
}

# ---------------------------------------------------------------------------
# Hash generation
# ---------------------------------------------------------------------------
generate_hashes() {
    info "Generating verification hashes..."

    # Collect all Linux packages produced in this session (current BASE_NAME only)
    local linux_pkgs=()
    for ext in deb rpm pkg.tar.zst; do
        for f in "${OUT_DIR}/${BASE_NAME}".*.${ext} "${OUT_DIR}/${BASE_NAME}".${ext}; do
            [[ -f "$f" ]] && linux_pkgs+=("$(basename "${f}")")
        done
    done
    # Static binary (no extension)
    local static_bin="${OUT_DIR}/${BASE_NAME}_linux"
    [[ -f "${static_bin}" ]] && linux_pkgs+=("$(basename "${static_bin}")")

    # Fallback: match any file built with current BASE_NAME prefix
    if [[ ${#linux_pkgs[@]} -eq 0 ]]; then
        while IFS= read -r -d '' f; do
            linux_pkgs+=("$(basename "${f}")")
        done < <(find "${OUT_DIR}" -maxdepth 1 -name "${BASE_NAME}*" \
                   \( -name "*.deb" -o -name "*.rpm" \
                   -o -name "*.pkg.tar.zst" -o -name "*_linux" \) \
                   -print0 2>/dev/null)
    fi

    if [[ ${#linux_pkgs[@]} -eq 0 ]]; then
        warn "No Linux packages found matching ${BASE_NAME}* — skipping hash generation"
        return
    fi

    (cd "${OUT_DIR}" && sha256sum "${linux_pkgs[@]}" > SHA256SUMS)
    (cd "${OUT_DIR}" && sha512sum "${linux_pkgs[@]}" > SHA512SUMS)
    success "Hashes → ${OUT_DIR}/SHA256SUMS, SHA512SUMS"
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
echo ""
echo "┌─────────────────────────────────────────────────────────┐"
echo "│          Zymbol Packaging Builder                       │"
echo "└─────────────────────────────────────────────────────────┘"
echo ""

[[ "${build_deb}" == true ]]      && { command -v dpkg-deb &>/dev/null || warn "dpkg-deb not found — .deb build will fail (install: sudo apt install dpkg)"; }

[[ "${build_deb}" == true ]]      && build_deb_package
[[ "${build_rpm}" == true ]]      && build_rpm_package
[[ "${build_arch}" == true ]]     && build_arch_package
[[ "${build_static}" == true ]]   && build_static_package
[[ "${build_win}" == true ]]      && build_windows_package
[[ "${build_winmsi}" == true ]]   && build_windows_msi_package

[[ "${DO_HASHES}" == true ]] && generate_hashes

echo ""
echo "────────────────────────────────────────────────"
success "Done! Packages in: ${OUT_DIR}"
echo ""
ls -lh "${OUT_DIR}/${BASE_NAME}"* 2>/dev/null || true
echo ""
