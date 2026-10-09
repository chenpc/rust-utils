#!/usr/bin/env bash
# =============================================================================
# scripts/cleanup.sh — 把本地環境清乾淨（rust / qemu / guest 三個 env root）
#
#   預設刪除：每個 env root 裡的 debs、root、bin、stage、modules、stamp
#   保留：    每個 env root 裡的 manifest.tsv，以及 logs/
#            —— 這是研究資料（已知 link + 環境變數記錄）；有 manifest 才能不用 apt 重製。
#
# 用法：
#   ./scripts/cleanup.sh                    # 清三個 env root（保留 manifest）
#   ./scripts/cleanup.sh --keep-debs        # 保留已下載的 .deb（可離線重解）
#   ./scripts/cleanup.sh --all              # 連 manifest.tsv 與 logs/ 也刪（會先要求確認）
#   ./scripts/cleanup.sh --dry-run          # 只印會刪什麼
#   ./scripts/cleanup.sh rust|qemu|guest    # 只清某個 env
#
# 被 source 時也會把目前 shell 的環境變數恢復。
# =============================================================================

_self="${BASH_SOURCE[0]:-${(%):-%x}}"
source "$(dirname "$_self")/common.sh"

KEEP_DEBS=0
DELETE_ALL=0
DRY_RUN=0
ONLY=""
for a in "$@"; do
  case "$a" in
    --keep-debs) KEEP_DEBS=1 ;;
    --all)       DELETE_ALL=1 ;;
    --dry-run)   DRY_RUN=1 ;;
    rust|qemu|guest) ONLY=$a ;;
    -h|--help)   sed -n '2,22p' "$_self"; exit 0 ;;
    *)           echo "[cleanup] 未知參數：$a" >&2; exit 1 ;;
  esac
done

roots=()
if [ -n "$ONLY" ]; then
  roots=("$ONLY")
else
  roots=(rust qemu guest)
fi

targets=()
for r in "${roots[@]}"; do
  root="$(env_root "$r")"
  [ -d "$root" ] || continue

  if [ "$DELETE_ALL" = 1 ]; then
    targets+=("$root")
    continue
  fi

  [ "$KEEP_DEBS" = 1 ] || targets+=("$root/debs")
  for d in root bin stage modules cargo-home rustup-home .install-stamp guest.env; do
    [ -e "$root/$d" ] && targets+=("$root/$d")
  done
done

if [ "$DELETE_ALL" = 1 ] && [ -e "$RUST_LOG_DIR" ]; then
  targets+=("$RUST_LOG_DIR")
fi

if [ ${#targets[@]} = 0 ]; then
  echo "[cleanup] 沒有東西要刪"
  exit 0
fi

echo "[cleanup] 將刪除："
for t in "${targets[@]}"; do
  sz="$(du -sh "$t" 2>/dev/null | cut -f1)"
  printf '  %s  (%s)\n' "$t" "${sz:-?}"
done

if [ "$DRY_RUN" = 1 ]; then exit 0; fi

if [ "$DELETE_ALL" = 1 ]; then
  printf '[cleanup] --all 會連研究資料（各 env 的 manifest.tsv、logs/）一起刪掉。輸入 yes 確認：'
  read -r ans
  [ "$ans" = "yes" ] || { echo "[cleanup] 中止"; exit 1; }
fi

# guest-share/tests 會累積每個 applet 的 test binary（119 個 ≈ 1.1 GB）
if [ -d "$_dir/guest-share/tests" ]; then targets+=("$_dir/guest-share/tests"); fi

# 散落的 coverage 產出（host 端跑 test binary 時 LLVM_PROFILE_FILE 未設定會寫到 repo root）
for f in "$_dir"/*.profraw "$_dir"/*.profdata; do
  [ -e "$f" ] && targets+=("$f")
done

for t in "${targets[@]}"; do rm -rf "$t"; done

if [ -n "${BASH_SOURCE+x}" ] && [ "${BASH_SOURCE[0]}" != "$0" ]; then
  PATH="$ENV_ORIG_PATH"
  LD_LIBRARY_PATH="${ENV_ORIG_LD:-}"
  CARGO_HOME="${ENV_ORIG_CARGO_HOME:-}"
  RUSTUP_HOME="${ENV_ORIG_RUSTUP_HOME:-}"
  PS1="${ENV_ORIG_PS1:-$PS1}"
  trap - EXIT
  rust_record "cleanup"
  printf '[cleanup] 環境變數已恢復\n'
  return 0 2>/dev/null || exit 0
fi

rust_record "cleanup"
echo "[cleanup] 完成"
exit 0
