#!/usr/bin/env bash
# =============================================================================
# scripts/install_qemu.sh — 安裝 QEMU（同一套 deb 管線）
#
#   1. 探索（第一次）：apt-cache 解析依賴 → apt-get --print-uris → .qemu-env/manifest.tsv
#   2. 重製（之後）：照 manifest 用 curl 下載 → 驗 SHA512 → dpkg-deb -x
#
# 解開後：
#   .qemu-env/root/usr/bin/qemu-system-x86_64   主程式
#   .qemu-env/root/usr/share/qemu/              firmware（seabios、ipxe）
#
# firmware 路徑是寫死在 qemu 裡的，所以執行時要手動指定：
#   -bios <seabios.bin>
#   -net nic,romfile=<ipxe> 或 romfile= （關掉）
# =============================================================================

set -u
_self="${BASH_SOURCE[0]}"
source "$(dirname "$_self")/common.sh"

STAMP="$QEMU_ENV_ROOT/.install-stamp"

if [ ! -f "$STAMP" ] || [ "$RUST_FORCE" = 1 ]; then
  rust_log "[install:qemu] 安裝（或 RUST_FORCE 重裝）"
  pkg_fetch "$QEMU_ENV_ROOT" $QEMU_PKGS && pkg_extract "$QEMU_ENV_ROOT" || exit 1
  cut -f1 "$QEMU_ENV_ROOT/manifest.tsv" 2>/dev/null | grep -v '^#' | sort > "$STAMP"
  rust_log "[install:qemu] 完成，stamp → $STAMP"
else
  rust_log "[install:qemu] 已安裝 → 跳過（RUST_FORCE=1 可重裝）"
fi
exit 0
