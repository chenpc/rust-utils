#!/usr/bin/env bash
# =============================================================================
# scripts/install_rust.sh — 安裝 Rust（只做安裝，不改目前 shell 的環境）
#
#   1. 探索（只允許第一次）：apt-cache 解析依賴 → apt-get --print-uris 取得確切 URL/SHA512
#      → 寫入 manifest.tsv
#   2. 重製（之後一律走 curl）：照 manifest.tsv 的已知 link 下載 → 驗 SHA512 → dpkg-deb -x
#
# 為什麼能不用 root：
#   - apt-get --print-uris / download 寫到目前目錄，使用者權限即可
#   - dpkg-deb -x 解到自訂目錄，使用者權限即可
#   - 依賴閉包只抓「系統尚未安裝」的套件，已裝的 lib 直接用系統版
#
# 用法：./scripts/install_rust.sh
# =============================================================================

set -u
_self="${BASH_SOURCE[0]}"
source "$(dirname "$_self")/common.sh"

STAMP="$RUST_ENV_ROOT/.install-stamp"

if [ ! -f "$STAMP" ] || [ "$RUST_FORCE" = 1 ]; then
  rust_log "[install:rust] 安裝（或 RUST_FORCE 重裝）"
  pkg_fetch "$RUST_ENV_ROOT" $RUST_PKGS && pkg_extract "$RUST_ENV_ROOT" || exit 1
  cut -f1 "$RUST_ENV_ROOT/manifest.tsv" 2>/dev/null | grep -v '^#' | sort > "$STAMP"
  rust_log "[install:rust] 完成，stamp → $STAMP"
else
  rust_log "[install:rust] 已安裝 → 跳過（RUST_FORCE=1 可重裝）"
fi
exit 0
