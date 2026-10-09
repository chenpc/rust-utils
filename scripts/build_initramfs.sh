#!/usr/bin/env bash
# =============================================================================
# scripts/build_initramfs.sh — 組 guest 的 initramfs（cpio.gz）
#
#   /init               我們的 coreutils multi-call binary（argv0=init → init applet）
#   /bin/sh             symlink → /init（payload shebang #!/bin/sh）
#   /modules/*.ko       從 .guest-env/modules 複製（已解 zstd）
#
# 不使用 busybox：init 與 shell 都是 coreutils 內的 applet（見 src/applets/init.rs、sh.rs）。
# payload 放在 9p share（guest 內的 /mnt），不在 initramfs 裡，這樣改測試內容不用重組 initramfs。
# =============================================================================

set -u
_self="${BASH_SOURCE[0]}"
_dir="$(cd "$(dirname "$_self")/.." && pwd)"
source "$_dir/scripts/common.sh"

BIN="$_dir/target/debug/coreutils"
[ -x "$BIN" ] || { rust_log "[initramfs] 找不到 $BIN（先跑 cargo build）"; exit 1; }

STAGE="$GUEST_ENV_ROOT/stage"
OUT="$GUEST_ENV_ROOT/initramfs.cpio.gz"
rm -rf "$STAGE"
mkdir -p "$STAGE/bin" "$STAGE/modules" "$STAGE/proc" "$STAGE/sys" "$STAGE/dev" "$STAGE/tmp" "$STAGE/mnt"

cp "$BIN" "$STAGE/init"
chmod +x "$STAGE/init"
ln -s /init "$STAGE/bin/sh"

for f in "$GUEST_ENV_ROOT/modules"/*.ko; do
  [ -e "$f" ] || continue
  cp "$f" "$STAGE/modules/"
done

( cd "$STAGE" && find . | sed 's|^\./||' | cpio -o -H newc 2>/dev/null | gzip -9 > "$OUT" )
rust_log "[initramfs] → $OUT ($(du -h "$OUT" | cut -f1))"
exit 0
