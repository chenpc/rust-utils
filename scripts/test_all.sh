#!/usr/bin/env bash
# =============================================================================
# scripts/test_all.sh — 平行跑 cargo test（預設 4 workers），每個 test binary 在 qemu guest 內執行
#
#   usage: ./scripts/test_all.sh [-j N] [applet ...]
#
# cargo 本身是依序呼叫 runner，所以平行度由這個 driver 控制：
#   1. cargo test --no-run 先編好所有 test binary
#   2. 共用資源先準備好（coreutils binary、profraw 輸出目錄）
#   3. N 個 worker 同時開 guest 跑 test binary
#   4. 全部結束後逐個 applet 算 coverage
#
# 平行注意事項（實測過的限制）：
#   - 每個 guest 128M（GUEST_MEM），4 workers = 512M
#   - profraw 檔名是 <test-id>-%p.profraw，test-id 每個 binary 唯一 → 不會互相覆蓋
#   - guest-share/bin/coreutils 是共用檔，只在不存在時複製（避免半寫入）
#   - 同一個 applet 跑兩次會共用 logs/<applet>.coverage.txt 與 .test.json → 不該並行同一 applet
# =============================================================================

set -u
_self="${BASH_SOURCE[0]:-${(%):-%x}}"
_dir="$(cd "$(dirname "$_self")/.." 2>/dev/null && pwd || echo .)"
source "$_dir/scripts/common.sh"

start=$SECONDS
JOBS=4
apps=()
while [ $# -gt 0 ]; do
  case "$1" in
    -j) JOBS="$2"; shift 2 ;;
    -j*) JOBS="${1#-j}"; shift ;;
    *) apps+=("$1"); shift ;;
  esac
done

SHARE="${SHARE:-$_dir/guest-share}"
OUTSHARE="${OUTSHARE:-$SHARE/out}"
LOGDIR="${LOGDIR:-$RUST_LOG_DIR}"
TARGET="$_dir/target/debug"

mkdir -p "$SHARE/tests" "$SHARE/bin" "$OUTSHARE" "$LOGDIR"

for a in "${apps[@]}"; do
  cargo test --no-run --test "$a" >/dev/null 2>&1 || { echo "cargo test --no-run 失敗 ($a)" >&2; exit 1; }
done
phase_build=$((SECONDS - start))

# 共用資源先準備好，避免 worker 之間搶
if [ -x "$TARGET/coreutils" ]; then cp "$TARGET/coreutils" "$SHARE/bin/coreutils"; fi
rm -f "$OUTSHARE"/*.profraw "$_dir"/*.profraw
export LLVM_PROFILE_FILE="$OUTSHARE/%p.profraw"

bins=()
covnames=()
if [ ${#apps[@]} -gt 0 ]; then
  targets=("${apps[@]}")
else
  targets=()
  for f in "$_dir/tests"/*.rs; do targets+=("$(basename "$f" .rs)"); done
fi
for a in "${targets[@]}"; do
  # cargo 會把 test target 名稱的連字號換成底線（sha3-224sum → sha3_224sum-hash）
  n="${a//-/_}"
  b="$(ls -t "$TARGET/deps/$n-"* 2>/dev/null | head -1)"
  if [ -n "$b" ] && [ -x "$b" ]; then
    bins+=("$b")
    covnames+=("$a")
  fi
done

[ ${#bins[@]} -gt 0 ] || { echo "找不到 test binary（$TARGET/deps）" >&2; exit 1; }

rust_log "[parallel] ${#bins[@]} 個 test binary，workers=$JOBS，GUEST_MEM=${GUEST_MEM:-128M}"
running=0
for b in "${bins[@]}"; do
  "$_dir/scripts/qemu_runner.sh" "$b" > "$LOGDIR/$(basename "$b").out" 2>&1 &
  running=$((running + 1))
  if [ "$running" -ge "$JOBS" ]; then
    wait -n
    running=$((running - 1))
  fi
done
wait
phase_guest=$((SECONDS - start))
start=$SECONDS
running=0

pass=0; fail=0
for b in "${bins[@]}"; do
  id="$(basename "$b")"
  line="$(grep -a 'RESULT:' "$LOGDIR/$id.out" | head -1)"
  printf '%s\n' "${line:-no result}"
  case "$line" in
    *PASS*) pass=$((pass + 1)) ;;
    *) fail=$((fail + 1)) ;;
  esac
done
rust_log "[parallel] PASS=$pass FAIL=$fail"

# coverage 是瓶頸（llvm-profdata + llvm-cov 每個 applet 約 4s），也要平行
for i in "${!bins[@]}"; do
  b="${bins[$i]}"
  id="$(basename "$b")"
  applet="${covnames[$i]}"
  [ "$applet" = "coreutils" ] && continue
  "$_dir/scripts/coverage.sh" "$applet" > "$LOGDIR/$applet.cov.out" 2>&1 &
  running=$((running + 1))
  if [ "$running" -ge "$JOBS" ]; then
    wait -n
    running=$((running - 1))
  fi
done
wait
phase_cov=$((SECONDS - start))
for i in "${!bins[@]}"; do
  applet="${covnames[$i]}"
  [ "$applet" = "coreutils" ] && continue
  grep '\[cov:' "$LOGDIR/$applet.cov.out"
done

rust_log "[phase] build=${phase_build}s guest=${phase_guest}s coverage=${phase_cov}s (workers=$JOBS)"
[ "$fail" = 0 ] || exit 1
exit 0
