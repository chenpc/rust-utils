#!/usr/bin/env bash
# =============================================================================
# scripts/run-shell.sh — 開 guest 進入互動 shell，用來手動實驗 applet
#
#   usage: ./scripts/run-shell.sh [-c 'command'] [--log FILE]
#
# guest 內有：
#   /bin/sh                    我們的 shell（coreutils applet，argv0 分派）
#   /bin/<applet>              每個 applet 的符號連結 → /mnt/bin/coreutils（multi-call）
#   /mnt                     host 的 guest-share（唯讀 9p）
#   /out                     host 的 guest-share/out（可寫 9p）
#
# 沒有參數 → serial 直接接到 stdio，就是互動 shell（exit 關機）。
# -c 'command' → 在 guest shell 跑一條命令後關機，輸出寫到 logs/run-shell.log。
#
# 互動模式的 escape：Ctrl-a c 切到 QEMU monitor（Ctrl-a d 直接離開 qemu）。
# =============================================================================

set -u
_self="${BASH_SOURCE[0]:-${(%):-%x}}"
_dir="$(cd "$(dirname "$_self")/.." 2>/dev/null && pwd || echo .)"
source "$_dir/scripts/common.sh"

[ -f "$GUEST_ENV_ROOT/guest.env" ] && . "$GUEST_ENV_ROOT/guest.env"
INITRD="$GUEST_ENV_ROOT/initramfs.cpio.gz"
SHARE="${SHARE:-$_dir/guest-share}"
OUTSHARE="${OUTSHARE:-$SHARE/out}"
LOGDIR="${LOGDIR:-$RUST_LOG_DIR}"
TIMEOUT_SEC="${TIMEOUT_SEC:-120}"
GUEST_MEM="${GUEST_MEM:-128M}"
GUEST_SMP="${GUEST_SMP:-1}"

cmd=""
logfile=""
while [ $# -gt 0 ]; do
  case "$1" in
    -c) shift; cmd="${1:-}"; shift ;;
    --log) shift; logfile="${1:-}"; shift ;;
    -h|--help) sed -n '2,20p' "$_self"; exit 0 ;;
    *) echo "usage: $(basename "$_self") [-c 'command'] [--log FILE]" >&2; exit 2 ;;
  esac
done

[ -n "${KERNEL:-}" ] && [ -f "$KERNEL" ] || { echo "missing kernel（先跑 scripts/install_guest.sh）" >&2; exit 2; }
[ -f "$INITRD" ] || { echo "missing $INITRD（先跑 scripts/build_initramfs.sh）" >&2; exit 2; }

QEMU_BIN="$QEMU_ENV_ROOT/root/usr/bin/qemu-system-x86_64"
[ -x "$QEMU_BIN" ] || QEMU_BIN="$QEMU_ENV_ROOT/bin/qemu-system-x86_64"
[ -x "$QEMU_BIN" ] || QEMU_BIN="$(command -v qemu-system-x86_64 || true)"
[ -n "$QEMU_BIN" ] || { echo "missing qemu（先跑 scripts/install_qemu.sh）" >&2; exit 2; }

ACCEL="$(detect_accel)"
if [ "$ACCEL" = kvm ]; then CPUOPT=host; else CPUOPT=qemu64; fi

ROMDIR="$QEMU_ENV_ROOT/root/usr/share/qemu"
BIOS=""
for cand in "$QEMU_ENV_ROOT/root/usr/share/seabios/bios-256k.bin" "$GUEST_ENV_ROOT/root/usr/share/seabios/bios-256k.bin" "$ROMDIR/bios-256k.bin" "$ROMDIR/bios.bin"; do
  [ -f "$cand" ] && { BIOS="$cand"; break; }
done
[ -n "$BIOS" ] || { echo "missing seabios（先跑 scripts/install_qemu.sh）" >&2; exit 2; }

mkdir -p "$LOGDIR" "$SHARE" "$OUTSHARE" "$SHARE/bin"

# applet 清單：給 guest 內的 /bin 建符號連結（multi-call binary 靠 argv[0] 分派）
applets_file="$SHARE/applets.txt"
( cd "$_dir/src/applets" && ls *.rs | sed 's/\.rs$//' ) > "$applets_file"

# multi-call binary 必須是靜態連結（guest 沒有 libc）；coverage build 也算，但體積大
bin_src=""
for cand in "$_dir/target/debug/coreutils" "$_dir/target/release/coreutils"; do
  [ -x "$cand" ] && { bin_src="$cand"; break; }
done
if [ -n "$bin_src" ]; then
  case "$(file -b "$bin_src" 2>/dev/null)" in
    *static*) : ;;
    *) echo "warning: $bin_src 不是靜態連結，guest 內可能跑不起来" >&2 ;;
  esac
  cp "$bin_src" "$SHARE/bin/coreutils"
  printf '==> binary: %s → %s/bin/coreutils\n' "$bin_src" "$SHARE"
else
  echo "warning: 找不到 target/debug/coreutils（先 cargo build）" >&2
fi

append="console=ttyS0 rd.shell=1"
if [ -n "$cmd" ]; then
  printf '%s\n' "$cmd" > "$SHARE/shell_cmd.sh"
  append="$append rd.cmd=shell_cmd"
fi

if [ -z "$logfile" ]; then
  logfile="$LOGDIR/run-shell.log"
fi

if [ -n "$cmd" ]; then
  : > "$logfile"
  serial="file:$logfile"
  monitor="none"
else
  # 互動：serial 接 stdio，同時保留 monitor（Ctrl-a c）
  serial="mon:stdio"
  monitor=""
fi

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
  -append "$append"
  -serial "$serial"
  -no-reboot
  -fsdev "local,id=fsdev0,path=$SHARE,security_model=none,readonly=on"
  -device virtio-9p-pci,fsdev=fsdev0,mount_tag=hostshare
  -fsdev "local,id=fsdev2,path=$OUTSHARE,security_model=none"
  -device virtio-9p-pci,fsdev=fsdev2,mount_tag=hostout
)
[ -n "$monitor" ] && qemu_cmd+=(-monitor "$monitor")

printf '==> accel=%s cpu=%s append="%s"\n' "$ACCEL" "$CPUOPT" "$append"
printf '==> applets list: %s (%s 個)\n' "$applets_file" "$(wc -l < "$applets_file")"

start=$SECONDS
timeout --foreground -k 5 "$TIMEOUT_SEC" "${qemu_cmd[@]}"
rc=$?
elapsed=$(( SECONDS - start ))

if [ -n "$cmd" ]; then
  printf '==> qemu exited rc=%s after %ss\n' "$rc" "$elapsed"
  printf '==> log: %s\n' "$logfile"
  grep -a -n -E 'QEMU-(INIT|SHELL):' "$logfile" | sed 's/^/      /'
fi
exit $rc
