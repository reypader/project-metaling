#!/usr/bin/env bash
set -euo pipefail

# Synchronizes gitignored upstream reference repositories inside ./reference/.
# Pass --pinned to checkout the verified baseline commit hashes for each repository.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PINNED_MODE=false

for arg in "$@"; do
    case "$arg" in
        --pinned)
            PINNED_MODE=true
            ;;
        *)
            echo "Usage: $0 [--pinned]" >&2
            exit 1
            ;;
    esac
done

sync_repo() {
    local name="$1"
    local url="$2"
    local branch="$3"
    local pinned_commit="$4"
    local target_dir="${SCRIPT_DIR}/${name}"

    if [ ! -d "${target_dir}/.git" ]; then
        echo "[sync-references] Cloning ${name} (${branch}) into ${target_dir}..."
        git clone --branch "${branch}" "${url}" "${target_dir}"
    else
        echo "[sync-references] Fetching updates for ${name}..."
        git -C "${target_dir}" fetch origin "${branch}"
    fi

    if [ "${PINNED_MODE}" = true ]; then
        echo "[sync-references] Checking out pinned commit ${pinned_commit} for ${name}..."
        git -C "${target_dir}" checkout "${pinned_commit}"
    else
        echo "[sync-references] Updating ${name} on branch ${branch}..."
        git -C "${target_dir}" checkout "${branch}"
        git -C "${target_dir}" pull --ff-only origin "${branch}"
    fi
}

sync_repo \
    "rathena" \
    "https://github.com/rathena/rathena.git" \
    "master" \
    "d4b8e7b8f16061cc2496d377ac8f777ce72a4f39"

sync_repo \
    "Hercules" \
    "https://github.com/HerculesWS/Hercules.git" \
    "stable" \
    "04a422ce60402c731d9cff0724cca9a5bc3c53e9"

sync_repo \
    "rust-ro" \
    "https://github.com/nmeylan/rust-ro.git" \
    "master" \
    "14193fcd2a285a9690cf28c923d9af56fe9587a5"

sync_repo \
    "rathena-script-lang-interpreter" \
    "https://github.com/nmeylan/rathena-script-lang-interpreter.git" \
    "master" \
    "2ef77cac84ac1a12a13875a3d3bc6363be1e9303"

echo "[sync-references] All reference repositories are synchronized."
