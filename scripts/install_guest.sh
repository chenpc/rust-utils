#!/usr/bin/env bash
# =============================================================================
# scripts/install_guest.sh — 安裝 guest 端素材（kernel + busybox + 9p 模組）
#
# 同一套 deb 管線：第一次探索 → 之後 curl 重製。
#
# 為什麼 kernel 一定要自己抓：
#   /boot/vmlinuz-* 是 0600 root，非 root 讀不到；/lib/modules 同理需要解一份。
#
# Ubuntu 的 kernel 模組是 .ko.zst（zstd 壓縮），initramfs 裡的 insmod 只能吃未壓縮的 .ko，
# 所以這裡先把 9p 相關模組解壓到 .guest-env/modules。
# =============================================================================

set -u
_self="${BASH_SOURCE[0]}"
source "$(dirname "$_self")/common.sh"

STAMP="$GUEST_ENV_ROOT/.install-stamp"

if [ ! -f "$STAMP" ] || [ "$RUST_FORCE" = 1 ]; then
  rust_log "[install:guest] 安裝（或 RUST_FORCE 重裝）"
  pkg_fetch "$GUEST_ENV_ROOT" $GUEST_PKGS && pkg_extract "$GUEST_ENV_ROOT" || exit 1
  cut -f1 "$GUEST_ENV_ROOT/manifest.tsv" 2>/dev/null | grep -v '^#' | sort > "$STAMP"
  rust_log "[install:guest] 完成，stamp → $STAMP"
else
  rust_log "[install:guest] 已安裝 → 跳過（RUST_FORCE=1 可重裝）"
fi

# ---- 9p 模組：解 zstd ----
MODDIR="$GUEST_ENV_ROOT/modules"
mkdir -p "$MODDIR"
rust_log "[install:guest] 解壓 9p 模組 → $MODDIR"
found=0
for f in "$GUEST_ENV_ROOT/root"/usr/lib/modules/*/kernel/fs/9p/*.ko.zst \
         "$GUEST_ENV_ROOT/root"/usr/lib/modules/*/kernel/net/9p/*.ko.zst \
         "$GUEST_ENV_ROOT/root"/usr/lib/modules/*/kernel/fs/netfs/*.ko.zst; do
  [ -e "$f" ] || continue
  zstd -d -q -f "$f" -o "$MODDIR/$(basename "${f%.zst}")" && found=$((found+1))
done
if [ "$found" = 0 ]; then
  rust_log "[install:guest] 找不到 9p 模組（kernel 閉包內沒有？確認 GUEST_PKGS）"
  exit 1
fi
rust_log "[install:guest] 解壓 $found 個模組"

# ---- kernel 路徑 ----
K="$(find "$GUEST_ENV_ROOT/root" -name 'vmlinuz*' -print 2>/dev/null | head -1)"
[ -n "$K" ] || { rust_log "[install:guest] 找不到 vmlinuz"; exit 1; }
printf 'KERNEL=%s\n' "$K" > "$GUEST_ENV_ROOT/guest.env"
printf '[install:guest] kernel → %s\n' "$K"
exit 0
