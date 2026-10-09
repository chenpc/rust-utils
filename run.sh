#!/usr/bin/env bash
# =============================================================================
# run.sh — 開 guest 一次，跑 9p share 上的一個 payload
#
#   usage: ./run.sh <test-id>
#
# payload 放在 guest-share/tests/<test-id>（可執行）或 <test-id>.sh（由 guest 的 busybox sh 跑）。
# serial console 寫到 logs/<test-id>.log，裡面的 QEMU-INIT / QEMU-TEST 是判定依據。
# 退出碼：0 PASS、1 FAIL、3 TIMEOUT、2 用法錯誤。
#
# accel 自動偵測：/dev/kvm 可讀可寫就用 kvm，否則 tcg。
# =============================================================================

set -u
_self="${BASH_SOURCE[0]:-${(%):-%x}}"
_dir="$(cd "$(dirname "$_self")" 2>/dev/null && pwd || echo .)"
source "$_dir/scripts/common.sh"

[ -f "$GUEST_ENV_ROOT/guest.env" ] && . "$GUEST_ENV_ROOT/guest.env"
INITRD="$GUEST_ENV_ROOT/initramfs.cpio.gz"
SHARE="${SHARE:-$_dir/guest-share}"
OUTSHARE="${OUTSHARE:-$SHARE/out}"
LOGDIR="${LOGDIR:-$RUST_LOG_DIR}"
TIMEOUT_SEC="${TIMEOUT_SEC:-120}"
# 平行開多個 guest 時記憶體壓力比較大；128M 是實測可用值（88M 會無法啟動），留 headroom。
# 可用 GUEST_MEM 覆蓋。
GUEST_MEM="${GUEST_MEM:-128M}"
# libtest（cargo test 的 harness）在 binary 內用執行緒平行跑 test，預設 thread 數 = available_parallelism，
# 也就是 guest 看到的 CPU 數。-smp 1 時 guest 內只有 1 顆 CPU → test 實際上是依序跑。
# 平行度是「4 個 qemu 同時跑」，不是同一個 qemu 內跑 4 個 test。
# 所以每個 guest 只有 1 顆 CPU，且 libtest 要單執行緒（runner 會加 --test-threads=1）。
GUEST_SMP="${GUEST_SMP:-1}"

[ $# -eq 1 ] || { echo "usage: $(basename "$_self") <test-id>" >&2; exit 2; }
test_id=$1
case "$test_id" in ""|/*|*..*) echo "bad test id: $test_id" >&2; exit 2 ;; esac

[ -n "${KERNEL:-}" ] && [ -f "$KERNEL" ] || { echo "missing kernel（先跑 scripts/install_guest.sh）" >&2; exit 2; }
[ -f "$INITRD" ] || { echo "missing $INITRD（先跑 scripts/build_initramfs.sh）" >&2; exit 2; }
[ -x "$SHARE/tests/$test_id" ] || [ -f "$SHARE/tests/$test_id.sh" ] || {
  echo "missing payload $SHARE/tests/$test_id (or .sh)" >&2; exit 2; }

QEMU_BIN="$QEMU_ENV_ROOT/bin/qemu-system-x86_64"
[ -x "$QEMU_BIN" ] || QEMU_BIN="$(command -v qemu-system-x86_64 || true)"
[ -n "$QEMU_BIN" ] || { echo "missing qemu（先跑 scripts/install_qemu.sh）" >&2; exit 2; }

ACCEL="$(detect_accel)"
# -cpu host 只有 KVM 能用；TCG 要用通用 CPU
if [ "$ACCEL" = kvm ]; then CPUOPT=host; else CPUOPT=qemu64; fi

# romfile / firmware 路徑寫死在 qemu 裡（/usr/share/qemu），非 root 不能寫系統目錄。
# qemu 找 romfile 會先看「目前目錄」，所以執行前 cd 到解開後的 share 目錄。
ROMDIR="$QEMU_ENV_ROOT/root/usr/share/qemu"
BIOS="$QEMU_ENV_ROOT/root/usr/share/seabios/bios-256k.bin"
[ -d "$ROMDIR" ] || { echo "missing rom dir（先跑 scripts/install_qemu.sh）" >&2; exit 2; }
[ -f "$BIOS" ] || BIOS="$ROMDIR/bios-256k.bin"

mkdir -p "$LOGDIR" "$OUTSHARE"
log="$LOGDIR/$test_id.log"
: > "$log"

LD_LIBRARY_PATH="$QEMU_ENV_ROOT/root/usr/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export LD_LIBRARY_PATH
cd "$ROMDIR" || exit 2

qemu_cmd=(
  "$QEMU_BIN"
  -machine pc
  -accel "$ACCEL"
  -cpu "$CPUOPT"
  -m "$GUEST_MEM"
  -smp "$GUEST_SMP"
  -nodefaults
  -vga none
  -display none
  -bios "$BIOS"
  -kernel "$KERNEL"
  -initrd "$INITRD"
  -append "console=ttyS0 rd.test=$test_id"
  -serial "file:$log"
  -monitor none
  -no-reboot
  -fsdev "local,id=fsdev0,path=$SHARE,security_model=none,readonly=on"
  -device virtio-9p-pci,fsdev=fsdev0,mount_tag=hostshare
  -fsdev "local,id=fsdev1,path=$OUTSHARE,security_model=none"
  -device virtio-9p-pci,fsdev=fsdev1,mount_tag=hostout
)

printf '==> accel=%s cpu=%s\n' "$ACCEL" "$CPUOPT"
printf '==> qemu %s\n' "${qemu_cmd[*]}"
start=$SECONDS
timeout --foreground -k 5 "$TIMEOUT_SEC" "${qemu_cmd[@]}"
rc=$?
elapsed=$(( SECONDS - start ))

printf '==> qemu exited rc=%s after %ss\n' "$rc" "$elapsed"
printf '==> markers from %s:\n' "$log"
grep -a -n -E 'QEMU-INIT:|QEMU-TEST:' "$log" | sed 's/^/      /'

if grep -a -q 'QEMU-TEST: PASS' "$log"; then
  printf 'RESULT: PASS %s (%ss, log=%s)\n' "$test_id" "$elapsed" "$log"; exit 0
fi
if grep -a -q 'QEMU-TEST: FAIL' "$log"; then
  printf 'RESULT: FAIL %s (%ss, log=%s)\n' "$test_id" "$elapsed" "$log"; exit 1
fi
if [ "$rc" = 124 ] || [ "$rc" = 137 ]; then
  printf 'RESULT: TIMEOUT after %ss (log=%s)\n' "$TIMEOUT_SEC" "$log"; exit 3
fi
printf 'RESULT: UNKNOWN %s (no marker, qemu rc=%s, log=%s)\n' "$test_id" "$rc" "$log"
exit 1
