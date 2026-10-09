#!/usr/bin/env bash
# =============================================================================
# scripts/coverage.sh — 從 guest 寫回的 profraw 計算 applet 來源檔的 region coverage
#
#   usage: ./scripts/coverage.sh <applet>
#
# profraw 由 guest 內的 test binary 寫到可寫的 9p share（guest-share/out/），
# 檔名是 <test-id>-%p.profraw（LLVM_PROFILE_FILE）。
# 判定：region coverage ≥ COVERAGE_MIN（預設 80）。
# 輸出：logs/<applet>.coverage.txt、logs/<applet>.profdata、logs/<applet>.test.json
# =============================================================================

set -u
_self="${BASH_SOURCE[0]:-${(%):-%x}}"
_dir="$(cd "$(dirname "$_self")/.." 2>/dev/null && pwd || echo .)"
source "$_dir/scripts/common.sh"

[ $# -eq 1 ] || { echo "usage: $(basename "$_self") <applet>" >&2; exit 2; }
applet=$1
COVERAGE_MIN="${COVERAGE_MIN:-80}"
TEST_PROFILE="${TEST_PROFILE:-debug}"

LLVM_PROF="$LLVM_ENV_ROOT/bin/llvm-profdata"
LLVM_COV="$LLVM_ENV_ROOT/bin/llvm-cov"
[ -x "$LLVM_PROF" ] && [ -x "$LLVM_COV" ] || {
  echo "missing llvm tools（先跑 scripts/install_llvm.sh）" >&2; exit 2; }

SHARE="${SHARE:-$_dir/guest-share}"
OUTSHARE="${OUTSHARE:-$SHARE/out}"
LOGDIR="${LOGDIR:-$RUST_LOG_DIR}"
TARGET="$_dir/target/$TEST_PROFILE"

# cargo 把 test target 名稱的連字號換成底線（sha3-224sum → sha3_224sum-hash），
# guest 內的 profraw 檔名也用同樣的 id，所以查 binary 與 profraw 都要正規化。
binname="${applet//-/_}"
TESTBIN="$(ls -t "$TARGET/deps/$binname-"* 2>/dev/null | head -1)"
[ -n "$TESTBIN" ] && [ -x "$TESTBIN" ] || {
  echo "找不到 $applet 的 test binary（$TARGET/deps/$binname-*）" >&2; exit 1; }

found=0
for f in "$OUTSHARE/$binname-"*.profraw; do [ -e "$f" ] && found=$((found + 1)); done
if [ "$found" = 0 ]; then
  rust_log "[cov:$applet] 沒有 profraw（$OUTSHARE/$binname-*.profraw）"
  exit 1
fi

merged="$LOGDIR/$applet.profdata"
"$LLVM_PROF" merge -sparse "$OUTSHARE/$binname-"*.profraw -o "$merged" || {
  echo "llvm-profdata merge 失敗" >&2; exit 1; }

report="$("$LLVM_COV" report "$TESTBIN" -instr-profile="$merged" 2>/dev/null)"
printf '%s\n' "$report" > "$LOGDIR/$applet.coverage.txt"
row="$(printf '%s\n' "$report" | grep -E "src/applets/$applet\.rs" | head -1)"
region="$(printf '%s\n' "$row" | awk '{ gsub(/%/, "", $4); print $4 }')"

printf '%s\n' "$report" | head -4
rust_log "[cov:$applet] test binary = $TESTBIN"
rust_log "[cov:$applet] profraw = $found"
# 用完就清掉，避免 share 被 *.profraw 塞滿（每次 test run 都會產生新的）
rm -f "$OUTSHARE/$applet-"*.profraw
rust_log "[cov:$applet] region coverage（src/applets/$applet.rs）= ${region:-N/A}%（門檻 $COVERAGE_MIN）"

[ -n "$region" ] || { rust_log "[cov:$applet] coverage 讀不到"; exit 1; }
ok="$(awk -v v="$region" -v m="$COVERAGE_MIN" 'BEGIN { print (v+0 >= m+0) ? 1 : 0 }')"
[ "$ok" = 1 ] || { rust_log "[cov:$applet] coverage 不足（$region < $COVERAGE_MIN）"; exit 1; }
rust_log "[cov:$applet] PASS（coverage $region%）"
exit 0
