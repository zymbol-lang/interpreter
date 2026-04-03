#!/usr/bin/env bash
# publish-release.sh — Build, hash, and publish Zymbol Linux packages + VS Code extension
#                       to a GitHub Release.
#
# Usage: bash packaging/publish-release.sh [OPTIONS]
# Run from the repository root.
#
# Prerequisites:
#   - gh CLI authenticated (gh auth login)
#   - dpkg-deb, rpmbuild, zstd, appimagetool (see build-packages.sh)
#   - sha256sum, sha512sum

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${REPO_ROOT}/packaging/dist"
VERSION=""
DRAFT=false
SKIP_REBUILD=false
SKIP_AARCH64=false

# ---------------------------------------------------------------------------
# Colours
# ---------------------------------------------------------------------------
RED='\033[0;31m'; YELLOW='\033[1;33m'; GREEN='\033[0;32m'; CYAN='\033[0;36m'; NC='\033[0m'
info()    { echo -e "${CYAN}[INFO]${NC}  $*"; }
success() { echo -e "${GREEN}[OK]${NC}    $*"; }
warn()    { echo -e "${YELLOW}[WARN]${NC}  $*"; }
error()   { echo -e "${RED}[ERROR]${NC} $*" >&2; exit 1; }

# ---------------------------------------------------------------------------
# Usage
# ---------------------------------------------------------------------------
usage() {
    cat <<EOF
Usage: bash packaging/publish-release.sh [OPTIONS]

Options:
  --version  X.Y.Z    Override version (default: read from interpreter/Cargo.toml)
  --draft              Create release as draft (requires manual publish on GitHub)
  --no-rebuild         Skip build step; use existing dist/ files (still renames + hashes)
  --no-aarch64         Skip aarch64 build even if cross-compiler is available
  -h, --help           Show this help

Examples:
  bash packaging/publish-release.sh --draft
  bash packaging/publish-release.sh --version 0.0.3
  bash packaging/publish-release.sh --no-rebuild
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version)    VERSION="$2";      shift 2 ;;
        --draft)      DRAFT=true;        shift ;;
        --no-rebuild) SKIP_REBUILD=true; shift ;;
        --no-aarch64) SKIP_AARCH64=true; shift ;;
        -h|--help)    usage; exit 0 ;;
        *) error "Unknown option: $1" ;;
    esac
done

# ---------------------------------------------------------------------------
# Checks
# ---------------------------------------------------------------------------
command -v gh &>/dev/null || error "'gh' CLI not found. Install: https://cli.github.com"
gh auth status &>/dev/null || error "Not authenticated with gh. Run: gh auth login"

# ---------------------------------------------------------------------------
# Detect version
# ---------------------------------------------------------------------------
if [[ -z "${VERSION}" ]]; then
    CARGO_TOML="${REPO_ROOT}/Cargo.toml"
    [[ -f "${CARGO_TOML}" ]] || error "Cargo.toml not found — run from interpreter/"
    VERSION=$(grep '^version' "${CARGO_TOML}" | head -1 | cut -d'"' -f2)
    [[ -n "${VERSION}" ]] || error "Could not parse version from Cargo.toml"
fi
TAG="v${VERSION}"
info "Version: ${VERSION}  Tag: ${TAG}"

# ---------------------------------------------------------------------------
# Banner
# ---------------------------------------------------------------------------
echo ""
echo "┌─────────────────────────────────────────────────────────┐"
echo "│          Zymbol Release Publisher                       │"
echo "└─────────────────────────────────────────────────────────┘"
echo ""

# ---------------------------------------------------------------------------
# Step 1 — Clean dist/ (Linux packages only, keep Windows)
# ---------------------------------------------------------------------------
info "Step 1: Cleaning Linux packages from dist/..."
mkdir -p "${OUT_DIR}"
rm -f "${OUT_DIR}"/*.deb \
      "${OUT_DIR}"/*.rpm \
      "${OUT_DIR}"/*.pkg.tar.zst \
      "${OUT_DIR}"/*_linux \
      "${OUT_DIR}"/SHA256SUMS \
      "${OUT_DIR}"/SHA512SUMS
success "dist/ cleaned (Windows .msi / .exe untouched)"

# ---------------------------------------------------------------------------
# Step 2 — Build Linux packages
# ---------------------------------------------------------------------------
BUILD_SCRIPT="${REPO_ROOT}/packaging/build-packages.sh"

if [[ "${SKIP_REBUILD}" == false ]]; then
    # x86_64
    info "Step 2a: Building x86_64 Linux packages..."
    bash "${BUILD_SCRIPT}" \
        --version "${VERSION}" \
        --arch x86_64 \
        --formats deb,rpm,arch,appimage \
        --no-hashes

    # aarch64 (optional — skip gracefully if cross-compiler absent)
    if [[ "${SKIP_AARCH64}" == false ]]; then
        if command -v aarch64-linux-gnu-gcc &>/dev/null; then
            info "Step 2b: Building aarch64 Linux packages..."
            bash "${BUILD_SCRIPT}" \
                --version "${VERSION}" \
                --arch aarch64 \
                --formats deb,rpm,arch,appimage \
                --no-hashes
        else
            warn "aarch64-linux-gnu-gcc not found — skipping aarch64 build."
            warn "  Install with: sudo apt install gcc-aarch64-linux-gnu"
            warn "  Then add to .cargo/config.toml:"
            warn "    [target.aarch64-unknown-linux-gnu]"
            warn "    linker = \"aarch64-linux-gnu-gcc\""
        fi
    fi
else
    info "Step 2: --no-rebuild set, using existing dist/ files."
fi

# ---------------------------------------------------------------------------
# Step 3 — Rename timestamped files to canonical names
# ---------------------------------------------------------------------------
info "Step 3: Renaming to canonical names (strip timestamp)..."

renamed=()
for f in "${OUT_DIR}"/zymbol_lang_v*.deb \
         "${OUT_DIR}"/zymbol_lang_v*.rpm \
         "${OUT_DIR}"/zymbol_lang_v*.pkg.tar.zst \
         "${OUT_DIR}"/zymbol_lang_v*_linux; do
    [[ -f "$f" ]] || continue
    base="$(basename "${f}")"

    # Match: zymbol_lang_v{VER}_{ARCH}_{TIMESTAMP}.ext  or  …_{TIMESTAMP}_linux
    # Target: zymbol_lang_v{VER}_{ARCH}.ext  or  …_{ARCH}_linux
    canonical="$(echo "${base}" | sed -E 's/_[0-9]{8}T[0-9]{4}(\.[^.]+(\.[^.]+)?|_linux)$/\1/')"

    if [[ "${base}" != "${canonical}" ]]; then
        mv -f "${f}" "${OUT_DIR}/${canonical}"
        info "  ${base} → ${canonical}"
        renamed+=("${canonical}")
    else
        renamed+=("${base}")
    fi
done

success "Canonical files: ${#renamed[@]}"

# ---------------------------------------------------------------------------
# Step 4 — Generate hashes (canonical Linux packages only)
# ---------------------------------------------------------------------------
info "Step 4: Generating SHA256SUMS / SHA512SUMS..."

linux_canonical=()
for f in "${OUT_DIR}"/zymbol_lang_v*.deb \
         "${OUT_DIR}"/zymbol_lang_v*.rpm \
         "${OUT_DIR}"/zymbol_lang_v*.pkg.tar.zst \
         "${OUT_DIR}"/zymbol_lang_v*_linux; do
    [[ -f "$f" ]] && linux_canonical+=("$(basename "${f}")")
done

if [[ ${#linux_canonical[@]} -gt 0 ]]; then
    (cd "${OUT_DIR}" && sha256sum "${linux_canonical[@]}" > SHA256SUMS)
    (cd "${OUT_DIR}" && sha512sum "${linux_canonical[@]}" > SHA512SUMS)
    success "SHA256SUMS and SHA512SUMS written (${#linux_canonical[@]} packages)"
else
    warn "No canonical Linux packages found — hash files not generated"
fi

# ---------------------------------------------------------------------------
# Helper: create or upload to a release in a given repo dir
# ---------------------------------------------------------------------------
gh_release_publish() {
    # gh_release_publish <repo_dir> <tag> <title> <notes_file> <draft_flag> <assets...>
    local repo_dir="$1"; shift
    local tag="$1";      shift
    local title="$1";    shift
    local notes="$1";    shift
    local draft="$1";    shift
    local assets=("$@")

    [[ ${#assets[@]} -gt 0 ]] || { warn "No assets for ${tag} in ${repo_dir} — skipping"; return; }

    info "  Assets (${#assets[@]}):"
    for a in "${assets[@]}"; do info "    $(basename "${a}")"; done

    (
        cd "${repo_dir}"
        if gh release view "${tag}" &>/dev/null 2>&1; then
            info "  Release ${tag} already exists — overwriting assets (--clobber)"
            gh release upload "${tag}" "${assets[@]}" --clobber
        else
            local extra=""
            [[ "${draft}" == true ]] && extra="--draft"
            gh release create "${tag}" \
                --title "${title}" \
                --notes-file "${notes}" \
                ${extra} \
                "${assets[@]}"
        fi
        gh release view "${tag}" --json url --jq '.url' 2>/dev/null || true
    )
}

# ---------------------------------------------------------------------------
# Step 5a — Publish Linux packages to interpreter repo
# ---------------------------------------------------------------------------
info "Step 5a: Publishing Linux packages → github.com/zymbol-lang/interpreter ${TAG}"

INTERP_NOTES="${REPO_ROOT}/packaging/RELEASE_NOTES_${VERSION}.md"
if [[ ! -f "${INTERP_NOTES}" ]]; then
    cat > "${INTERP_NOTES}" <<EOF
## Zymbol-Lang ${VERSION}

### Linux packages (x86_64 and aarch64)
- \`.deb\` — Debian / Ubuntu
- \`.rpm\` — Fedora / RHEL / openSUSE
- \`.pkg.tar.zst\` — Arch Linux
- \`_linux\` — Static binary (musl, no dependencies, any Linux)

### Verification
Download \`SHA256SUMS\` alongside your package and run:
\`\`\`bash
sha256sum --ignore-missing -c SHA256SUMS
\`\`\`

### Windows / macOS
Coming soon — signing certificates pending.
EOF
fi

interp_assets=()
for f in "${linux_canonical[@]+"${linux_canonical[@]}"}"; do
    interp_assets+=("${OUT_DIR}/${f}")
done
[[ -f "${OUT_DIR}/SHA256SUMS" ]] && interp_assets+=("${OUT_DIR}/SHA256SUMS")
[[ -f "${OUT_DIR}/SHA512SUMS" ]] && interp_assets+=("${OUT_DIR}/SHA512SUMS")

gh_release_publish \
    "${REPO_ROOT}" \
    "${TAG}" \
    "Zymbol-Lang ${VERSION}" \
    "${INTERP_NOTES}" \
    "${DRAFT}" \
    "${interp_assets[@]+"${interp_assets[@]}"}"

# ---------------------------------------------------------------------------
# Step 5b — Publish VS Code extension to vscode repo
# ---------------------------------------------------------------------------
info "Step 5b: Publishing VS Code extension → github.com/zymbol-lang/vscode"

VSIX_SRC=""
# Pick the newest .vsix by filename (lexicographic = chronological given timestamp naming)
while IFS= read -r -d '' f; do
    VSIX_SRC="${f}"
done < <(find "${REPO_ROOT}/../vscode" -maxdepth 1 -name "*.vsix" -print0 | sort -z)

if [[ -n "${VSIX_SRC}" ]]; then
    VSIX_VER="$(basename "${VSIX_SRC}" | grep -oP '(?<=zymbol-lang-)\d+\.\d+\.\d+')" || true
    VSIX_VER="${VSIX_VER:-0.1.0}"
    VSIX_TAG="v${VSIX_VER}"
    VSCODE_DIR="${REPO_ROOT}/../vscode"
    VSIX_CANONICAL="${VSCODE_DIR}/zymbol-lang-${VSIX_VER}.vsix"
    cp -f "${VSIX_SRC}" "${VSIX_CANONICAL}"
    success "VS Code extension staged: zymbol-lang-${VSIX_VER}.vsix"

    VSCODE_NOTES="${VSCODE_DIR}/RELEASE_NOTES_${VSIX_VER}.md"
    if [[ ! -f "${VSCODE_NOTES}" ]]; then
        cat > "${VSCODE_NOTES}" <<EOF
## Zymbol-Lang VS Code Extension ${VSIX_VER}

Syntax highlighting, semantic tokens, bracket matching, and LSP integration
(hover, go-to-definition, diagnostics).

### Install
\`\`\`bash
code --install-extension zymbol-lang-${VSIX_VER}.vsix
\`\`\`
Or: Extensions (Ctrl+Shift+X) → ··· → Install from VSIX…
EOF
    fi

    gh_release_publish \
        "${VSCODE_DIR}" \
        "${VSIX_TAG}" \
        "Zymbol-Lang VS Code Extension ${VSIX_VER}" \
        "${VSCODE_NOTES}" \
        "${DRAFT}" \
        "${VSIX_CANONICAL}"
else
    warn "No .vsix found in vscode/ — VS Code extension release skipped"
fi

echo ""
echo "────────────────────────────────────────────────"
success "All releases published!"
echo ""
