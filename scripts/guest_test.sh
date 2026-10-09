#!/usr/bin/env bash
# =============================================================================
# scripts/guest_test.sh — 跑某個 applet 的 cargo test（在 qemu guest 內）+ coverage
#
#   usage: ./scripts/guest_test.sh <applet>
#
# `cargo test` 本身就會在 guest 內跑：.cargo/config.toml 設了
#   runner = scripts/qemu_runner.sh（把 test binary 送進 9p share，開 guest 執行）
#   rustflags = -C target-feature=+crt-static -C instrument-coverage
#
# 判定：test 通過 + src/applets/<applet>.rs 的 region coverage ≥ COVERAGE_MIN（預設 80）。
# 輸出：logs/<test-id>.log（guest serial）、logs/<applet>.coverage.txt、logs/<applet>.test.json
# =============================================================================

set -u
_self="${BASH_SOURCE[0]:-${(%):-%x}}"
_dir="$(cd "$(dirname "$_self")/.." 2>/dev/null && pwd || echo .)"
source "$_dir/scripts/common.sh"

[ $# -eq 1 ] || { echo "usage: $(basename "$_self") <applet>" >&2; exit 2; }
applet=$1
COVERAGE_MIN="${COVERAGE_MIN:-80}"
SHARE="${SHARE:-$_dir/guest-share}"
OUTSHARE="${OUTSHARE:-$SHARE/out}"
LOGDIR="${LOGDIR:-$RUST_LOG_DIR}"

# 每次只收這次的 profraw，否則 coverage 會混入之前跑過的結果
rm -f "$OUTSHARE"/*.profraw

rust_log "[test:$applet] cargo test（runner → qemu guest）"
rc=0
cargo test --test "$applet" || rc=$?

test_json="$LOGDIR/$applet.test.json"
if [ "$rc" != 0 ]; then
  printf '{"applet":"%s","test_rc":%s,"coverage":null,"coverage_min":%s}\n' \
    "$applet" "$rc" "$COVERAGE_MIN" > "$test_json"
  rust_log "[test:$applet] FAIL（test rc=$rc）"
  exit 1
fi

"$_dir/scripts/coverage.sh" "$applet" || exit 1

# coverage.sh 已寫 coverage.txt；這裡補上 test_rc
printf '{"applet":"%s","test_rc":0,"coverage":%s,"coverage_min":%s}\n' \
  "$applet" "$(grep -E "src/applets/$applet\.rs" "$LOGDIR/$applet.coverage.txt" | head -1 | awk '{gsub(/%/,"");print $4}')" \
  "$COVERAGE_MIN" > "$test_json"
exit 0
