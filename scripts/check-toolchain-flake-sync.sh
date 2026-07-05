#!/usr/bin/env bash
set -euo pipefail

if [ -n "${PRE_COMMIT_FROM_REF:-}" ] && [ -n "${PRE_COMMIT_TO_REF:-}" ]; then
    # CI PR mode: prek passes the base...head range.
    changed=$(git diff --name-only "${PRE_COMMIT_FROM_REF}...${PRE_COMMIT_TO_REF}")
else
    # Local commit: check the staged set.
    changed=$(git diff --cached --name-only)
fi

# No toolchain change in scope (also covers --all-files replay, where
# nothing is staged and the nix-checks CI job is the actual gate) -> pass.
grep -qx "rust-toolchain.toml" <<< "$changed" || exit 0

grep -qx "flake.lock" <<< "$changed" || {
    echo "rust-toolchain.toml changed without flake.lock -- run nix flake update rust-overlay"
    exit 1
}
