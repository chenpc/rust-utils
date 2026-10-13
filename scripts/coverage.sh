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
# 來源檔映射：dir/vdir 實作於 ls.rs（COVERAGE_SRC）；COVERAGE_EXTRA_TESTS 列出
# 共用的其他 test binary（ls 的 coverage 由 ls+ls_extra+dir+vdir 四個 binary 合併）。
src_file="${COVERAGE_SRC:-src/applets/$applet.rs}"
extra_tests="${COVERAGE_EXTRA_TESTS:-}"
case "$applet" in
  ls) src_file=src/applets/ls.rs; extra_tests="ls_extra ls_coverage dir vdir" ;;
  ls_extra | dir | vdir | ls_coverage) src_file=src/applets/ls.rs; extra_tests="ls ls_extra ls_coverage dir vdir" ;;
esac
TESTBIN="$(ls -t "$TARGET/deps/$binname-"* 2>/dev/null | head -1)"
# dir/vdir/ls_extra/ls_extra 與 ls 共用同一 lib 代碼：coverage 以 ls test binary
# ＋四者合併 profraw 為準（各 binary 自身的 coverage map 只含自己 link 到的代碼，
# 分开報會低估共用代碼）。
case "$applet" in
  ls_extra | dir | vdir | ls_coverage)
    TESTBIN="$(ls -t "$TARGET/deps/ls-"* 2>/dev/null | head -1)" ;;
esac
[ -n "$TESTBIN" ] && [ -x "$TESTBIN" ] || {
  echo "找不到 $applet 的 test binary（$TARGET/deps/$binname-*）" >&2; exit 1; }

merged="$LOGDIR/$applet.profdata"
binlist=("$binname")
for x in $extra_tests; do binlist+=("${x//-/_}"); done

found=0
raws=()
for b in "${binlist[@]}"; do
  for f in "$OUTSHARE/$b-"*.profraw; do
    [ -e "$f" ] && { raws+=("$f"); found=$((found + 1)); }
  done
done
if [ "$found" = 0 ]; then
  # 共用 binary 的 profraw 可能已被同组的 coverage 跑合并并清掉（ls/dir/vdir/ls_extra）；
  # 若本 applet（或 ls 主档）已有近期 merged profdata（<15 分鐘）則沿用。
  fam="$LOGDIR/ls.profdata"
  if [ -f "$merged" ] && [ "$(( $(date +%s) - $(stat -c %Y "$merged") ))" -lt 3600 ]; then
    rust_log "[cov:$applet] 沿用既有 merged profdata（共用 profraw 已清）"
  elif [ -f "$fam" ] && [ "$(( $(date +%s) - $(stat -c %Y "$fam") ))" -lt 3600 ]; then
    merged="$fam"
    rust_log "[cov:$applet] 沿用 ls.profdata（共用 profraw 已清）"
  else
    rust_log "[cov:$applet] 沒有 profraw（$OUTSHARE/$binname-*.profraw）"
    exit 1
  fi
else
  merged="$LOGDIR/$applet.profdata"
  "$LLVM_PROF" merge -sparse "${raws[@]}" -o "$merged" || {
    echo "llvm-profdata merge 失敗" >&2; exit 1; }
fi

report="$("$LLVM_COV" report "$TESTBIN" -instr-profile="$merged" 2>/dev/null)"
printf '%s\n' "$report" > "$LOGDIR/$applet.coverage.txt"
row="$(printf '%s\n' "$report" | grep -E "$src_file" | head -1)"
region="$(printf '%s\n' "$row" | awk '{ gsub(/%/, "", $4); print $4 }')"

printf '%s\n' "$report" | head -4
rust_log "[cov:$applet] test binary = $TESTBIN"
rust_log "[cov:$applet] profraw = $found"
# 用完就清掉，避免 share 被 *.profraw 塞滿（每次 test run 都會產生新的）
for b in "${binlist[@]}"; do rm -f "$OUTSHARE/$b-"*.profraw; done
rust_log "[cov:$applet] region coverage（$src_file）= ${region:-N/A}%（門檻 $COVERAGE_MIN）"

[ -n "$region" ] || { rust_log "[cov:$applet] coverage 讀不到"; exit 1; }
ok="$(awk -v v="$region" -v m="$COVERAGE_MIN" 'BEGIN { print (v+0 >= m+0) ? 1 : 0 }')"
[ "$ok" = 1 ] || { rust_log "[cov:$applet] coverage 不足（$region < $COVERAGE_MIN）"; exit 1; }
rust_log "[cov:$applet] PASS（coverage $region%）"
exit 0
