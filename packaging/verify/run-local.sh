#!/usr/bin/env bash
# run-local.sh — Run the release gate on this machine, the way CI runs it.
#
# Builds the binary with the packaged feature set, builds the .deb, and verifies
# it inside a pristine debian:12 container. Same script CI calls, same image, so
# a failure here is the failure CI would report.
#
# Usage: bash packaging/verify/run-local.sh [OPTIONS]
#
# Options:
#   --host-build   Compile with the host toolchain (see "Where it compiles")
#   --no-build     Reuse the binary and .deb from a previous run
#   --scope SCOPE  smoke | full   (default: full)
#   --engine ENG   podman | docker   (default: whichever is installed)
#   -h, --help
#
# Requires podman or docker:
#   sudo apt install podman        # rootless, no daemon
#
# Where it compiles
# -----------------
# A binary can run on its build machine's glibc or newer, never older, and the
# package now declares that floor honestly (`Depends: libc6 (>= X.Y)`). So on a
# host newer than the verification image — Debian 13 is glibc 2.41, debian:12 is
# 2.36 — a host build produces a package that correctly refuses to install in the
# container, and the gate is red for a reason that has nothing to do with your
# changes.
#
# So by default the compile happens in a container whose glibc matches the
# verification image. --host-build opts out, which is fine when your host glibc
# is not newer than the image's.
#
# Note on the binary: this never writes target/release/, so a `zymbol` on your
# PATH pointing at the development build keeps its std/db.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DO_BUILD=true
HOST_BUILD=false
SCOPE="full"
ENGINE=""
RUST_TARGET="x86_64-unknown-linux-gnu"

# debian:12 is the verification image; rust:1-bookworm is the same distro with a
# toolchain on top, so a binary built there installs and runs in it. CI compiles
# on ubuntu-22.04 (glibc 2.35), a hair older still — either way the package ends
# up with a floor debian:12 satisfies.
VERIFY_IMAGE="debian:12"
VERIFY_IMAGE_GLIBC="2.36"
BUILD_IMAGE="docker.io/library/rust:1-bookworm"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --host-build) HOST_BUILD=true; shift ;;
        --no-build)   DO_BUILD=false;  shift ;;
        --scope)      SCOPE="$2";      shift 2 ;;
        --engine)     ENGINE="$2";     shift 2 ;;
        -h|--help)    sed -n '2,33p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) echo "Unknown option: $1" >&2; exit 2 ;;
    esac
done

if [[ -z "${ENGINE}" ]]; then
    if command -v podman >/dev/null;   then ENGINE=podman
    elif command -v docker >/dev/null; then ENGINE=docker
    else
        echo "Neither podman nor docker found. Install one:" >&2
        echo "  sudo apt install podman" >&2
        exit 2
    fi
fi

cd "${REPO_ROOT}"
VERSION=$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)
DEB="packaging/dist/zymbol_lang_v${VERSION}_x86_64.deb"

# Host build into target/<triple>/, container build into target/container/ — kept
# apart so the two toolchains do not invalidate each other's incremental state.
HOST_BIN="target/${RUST_TARGET}/release/zymbol"
CONTAINER_BIN="target/container/${RUST_TARGET}/release/zymbol"

if [[ "${DO_BUILD}" == true ]]; then
    HOST_GLIBC="$(ldd --version | head -1 | grep -o '[0-9]\+\.[0-9]\+$' || echo '')"

    if [[ "${HOST_BUILD}" != true ]] \
       && [[ -n "${HOST_GLIBC}" ]] \
       && [[ "$(printf '%s\n%s\n' "${HOST_GLIBC}" "${VERIFY_IMAGE_GLIBC}" | sort -V | tail -1)" == "${HOST_GLIBC}" ]] \
       && [[ "${HOST_GLIBC}" != "${VERIFY_IMAGE_GLIBC}" ]]
    then
        echo "==> Host glibc ${HOST_GLIBC} is newer than ${VERIFY_IMAGE}'s ${VERIFY_IMAGE_GLIBC}"
        echo "    Compiling in ${BUILD_IMAGE} so the package matches what CI produces."
        echo "    (--host-build to compile here instead)"
        BIN="${CONTAINER_BIN}"
        "${ENGINE}" run --rm \
            -v "${REPO_ROOT}:/workspace" \
            --workdir /workspace \
            -e CARGO_TARGET_DIR=/workspace/target/container \
            -e CARGO_HOME=/workspace/target/container/cargo-home \
            "${BUILD_IMAGE}" \
            cargo build --release --no-default-features --target "${RUST_TARGET}"
    else
        echo "==> Building with the host toolchain (--no-default-features, as packaged)"
        BIN="${HOST_BIN}"
        cargo build --release --no-default-features --target "${RUST_TARGET}"
    fi

    echo "==> Building .deb from ${BIN}"
    bash packaging/build-packages.sh \
        --arch x86_64 --formats deb --binary "${BIN}" --no-timestamp --no-hashes
fi

[[ -f "${DEB}" ]] || { echo "Not found: ${DEB} (drop --no-build?)" >&2; exit 1; }

echo "==> Verifying ${DEB} in ${VERIFY_IMAGE} via ${ENGINE}"
exec "${ENGINE}" run --rm \
    -v "${REPO_ROOT}:/workspace" \
    --workdir /workspace \
    "${VERIFY_IMAGE}" \
    bash packaging/verify/verify-deb.sh --deb "${DEB}" --scope "${SCOPE}"
