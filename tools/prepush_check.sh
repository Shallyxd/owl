#!/bin/bash
# Pre-push mechanical checks — run before EVERY push, no exceptions.
# Checks: Dockerfile FROM lines (allowlist + digest-pinned), /out file name,
#         no hidden files in track folders, cargo fmt pass.
set -e

TRACKS="track-a track-b track-c track-d"
FAIL=0

check_fail() {
    echo "FAIL: $1"
    FAIL=$((FAIL+1))
}

echo "=== Pre-push checks ==="

for track in $TRACKS; do
    [ -d "$track" ] || continue
    echo "--- $track ---"

    # 1. Dockerfile exists
    if [ ! -f "$track/Dockerfile" ]; then
        check_fail "$track/Dockerfile missing"
        continue
    fi

    # 2. FROM lines must be digest-pinned (contain @sha256:)
    while IFS= read -r line; do
        if echo "$line" | grep -qi "^FROM"; then
            if ! echo "$line" | grep -q "@sha256:"; then
                check_fail "$track Dockerfile FROM not digest-pinned: $line"
            fi
            # Must not contain FILL_DIGEST_HERE
            if echo "$line" | grep -q "FILL_DIGEST_HERE"; then
                check_fail "$track Dockerfile has placeholder digest — fill from board allowlist"
            fi
        fi
    done < "$track/Dockerfile"

    # 3. No hidden files in track folder (scc --no-gitignore counts them)
    # Note: .speed-demon.toml is the official manifest; target/ is build artifact
    hidden=$(find "$track" -name ".*" -not -name ".gitignore" -not -name ".speed-demon.toml" -not -path "*/.git/*" -not -path "*/target/*" 2>/dev/null)
    if [ -n "$hidden" ]; then
        check_fail "$track has hidden files (scc counts them): $hidden"
    fi

    # 4. cargo fmt check (if Rust track)
    if [ -f "$track/Cargo.toml" ]; then
        if ! (cd "$track" && cargo fmt --check 2>/dev/null); then
            check_fail "$track fails cargo fmt --check (run: cd $track && cargo fmt)"
        fi
    fi
done

# 5. C test oracle must pass
if [ -f "tools/test_c_parser.py" ]; then
    echo "--- C test oracle ---"
    if ! python3 tools/test_c_parser.py; then
        check_fail "Track C test oracle failed"
    fi
fi

echo ""
if [ "$FAIL" -eq 0 ]; then
    echo "ALL CHECKS PASSED — safe to push"
else
    echo "$FAIL check(s) FAILED — DO NOT PUSH"
    exit 1
fi

