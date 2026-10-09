#!/usr/bin/env bash
# =============================================================================
# scripts/qemu_runner.sh — cargo 的 test runner：讓 `cargo test` 直接在 qemu guest 內跑
#
# cargo 會這樣呼叫：  runner <test-binary> [cargo 傳給 binary 的參數...]
#
# 做法：把 test binary 放到 9p share，寫一個 .sh payload 設定 guest 內的環境變數，
#       再用 run.sh 開 guest 執行，最後把 guest 內的 exit code 原樣回傳給 cargo。
#
# guest 內沒有 toolchain 也沒有 libc，所以 binary 必須是靜態連結
# （.cargo/config.toml 裡的 rustflags 已經帶 -C target-feature=+crt-static）。
# coverage：LLVM_PROFILE_FILE 寫到可寫的 9p share（/out），回 host 由 scripts/coverage.sh 計算。
# =============================================================================

set -u
_self="${BASH_SOURCE[0]:-${(%):-%x}}"
_dir="$(cd "$(dirname "$_self")/.." 2>/dev/null && pwd || echo .)"
source "$_dir/scripts/common.sh"

bin="$1"; shift
[ -n "$bin" ] || { echo "qemu_runner: no binary given" >&2; exit 2; }

SHARE="${SHARE:-$_dir/guest-share}"
OUTSHARE="${OUTSHARE:-$SHARE/out}"
mkdir -p "$SHARE/tests" "$SHARE/bin" "$OUTSHARE"

id="$(basename "$bin")"
case "$id" in ""|/*|*..*) echo "qemu_runner: bad test id: $id" >&2; exit 2 ;; esac

cp "$bin" "$SHARE/tests/$id"

# multi-call binary 給 e2e test 用（testutil::run_bin 讀 COREUTILS_BIN）
# 平行跑時這份是共用資源：只在不存在時複製，避免半寫入的 binary
mainbin="$(dirname "$bin")"
[ "$(basename "$mainbin")" = "deps" ] && mainbin="$(dirname "$mainbin")"
for cand in "$mainbin/coreutils" "$_dir/target/debug/coreutils" "$_dir/target/release/coreutils"; do
  if [ -x "$cand" ]; then
    # 共用資源：只在「不存在」或「來源比目標新」時複製，避免 e2e test 讀到舊 binary
    if [ ! -e "$SHARE/bin/coreutils" ] || [ "$cand" -nt "$SHARE/bin/coreutils" ]; then
      cp "$cand" "$SHARE/bin/coreutils"
    fi
    break
  fi
done

args=""
for a in "$@"; do args="$args $(printf '%q' "$a")"; done
# 平行度是「4 個 qemu」，不是同一個 qemu 內 4 個 test：guest 只有 1 顆 CPU，libtest 單執行緒
if [ -z "$args" ]; then args=" --test-threads=1"; fi

cat > "$SHARE/tests/$id.sh" << EOF
#!/bin/busybox sh
export COREUTILS_BIN=/mnt/bin/coreutils
export COREUTILS_WORK=/tmp
export LLVM_PROFILE_FILE="/out/$id-%p.profraw"
/mnt/tests/$id$args
EOF
chmod +x "$SHARE/tests/$id.sh"

"$_dir/run.sh" "$id"
rc=$?
exit $rc
