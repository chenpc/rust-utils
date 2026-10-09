#!/usr/bin/env bash
# =============================================================================
# scripts/install_llvm.sh — 安裝 coverage 工具（llvm-profdata / llvm-cov）
#
#   同一套「非 root 裝 deb」架構：探索（apt-get --print-uris，只第一次）→ manifest.tsv
#   → 之後一律 curl + SHA512 → dpkg-deb -x。
#
#   rustc 的 -C instrument-coverage 只產出 profraw，解析需要 llvm-profdata/llvm-cov，
#   這兩個不在 rust 套件內，所以要另外裝。
#
# 用法：./scripts/install_llvm.sh
# =============================================================================

set -u
_self="${BASH_SOURCE[0]}"
source "$(dirname "$_self")/common.sh"

STAMP="$LLVM_ENV_ROOT/.install-stamp"

if [ ! -f "$STAMP" ] || [ "$RUST_FORCE" = 1 ]; then
  rust_log "[install:llvm] 安裝（或 RUST_FORCE 重裝）"
  pkg_fetch "$LLVM_ENV_ROOT" $LLVM_PKGS && pkg_extract "$LLVM_ENV_ROOT" || exit 1
  cut -f1 "$LLVM_ENV_ROOT/manifest.tsv" 2>/dev/null | grep -v '^#' | sort > "$STAMP"
  rust_log "[install:llvm] 完成，stamp → $STAMP"
else
  rust_log "[install:llvm] 已安裝 → 跳過（RUST_FORCE=1 可重裝）"
fi
exit 0
