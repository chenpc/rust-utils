#!/usr/bin/env bash
# =============================================================================
# scripts/common.sh — 共用路徑與「非 root 裝 deb」的通用函式
# 不直接執行，只被 source。
#
# 通用流程（每個 env root 各一組）：
#   pkg_discover <root> <pkgs...>   探索：apt-get --print-uris → <root>/manifest.tsv
#   pkg_fetch    <root>             重製：照 manifest 用 curl 下載 + 驗 SHA512
#   pkg_extract  <root>             解開：dpkg-deb -x → <root>/root，建 <root>/bin
#
# apt 只允許出現在「探索」這一步；之後一律走 curl。
# =============================================================================

_c_self="${BASH_SOURCE[0]:-${(%):-%x}}"
_c_base="$(cd "$(dirname "$_c_self")/.." 2>/dev/null && pwd || echo .)"

# ---- 共用路徑 ----
RUST_ENV_ROOT="${RUST_ENV_ROOT:-$_c_base/.rust-env}"
QEMU_ENV_ROOT="${QEMU_ENV_ROOT:-$_c_base/.qemu-env}"

RUST_PKGS="${RUST_PKGS:-rustc cargo}"
QEMU_PKGS="${QEMU_PKGS:-qemu-system-x86}"
GUEST_PKGS="${GUEST_PKGS:-linux-image-7.0.0-38-generic linux-modules-7.0.0-38-generic busybox libc6}"
# 這些套件即使系統已安裝也一定要抓：/boot 與 /lib/modules 是 root 權限（0600），
# 非 root 讀不到；libc6 則是 guest initramfs 需要自帶 runtime（busybox 是動態連結）。
PKG_FORCE="${PKG_FORCE:-linux-image-7.0.0-38-generic linux-modules-7.0.0-38-generic libc6}"
GUEST_ENV_ROOT="${GUEST_ENV_ROOT:-$_c_base/.guest-env}"
LLVM_ENV_ROOT="${LLVM_ENV_ROOT:-$_c_base/.llvm-env}"
# coverage 需要 llvm-profdata / llvm-cov（rustc 的 -C instrument-coverage 產出 profraw，
# 但解析工具不在 rust 套件裡）。版本要與 rustc 內建 LLVM 相容，所以用 llvm-21。
LLVM_PKGS="${LLVM_PKGS:-llvm-21}"

env_root() {
  case "$1" in
    rust)  printf '%s\n' "$RUST_ENV_ROOT" ;;
    qemu)  printf '%s\n' "$QEMU_ENV_ROOT" ;;
    guest) printf '%s\n' "$GUEST_ENV_ROOT" ;;
    llvm)  printf '%s\n' "$LLVM_ENV_ROOT" ;;
    *)     return 1 ;;
  esac
}

RUST_FETCH="${RUST_FETCH:-curl}"          # curl（重製路徑）| apt（僅探索）
RUST_ALL_DEPS="${RUST_ALL_DEPS:-0}"
RUST_OFFLINE="${RUST_OFFLINE:-0}"
RUST_FORCE="${RUST_FORCE:-0}"
RUST_QUIET="${RUST_QUIET:-0}"

RUST_LOG_DIR="${RUST_LOG_DIR:-$_c_base/logs}"
RUST_ENV_LOG="${RUST_ENV_LOG:-$RUST_LOG_DIR/env.jsonl}"

rust_log() { [ "$RUST_QUIET" = 1 ] || printf '%s\n' "$*" >&2; }
rust_installed() { dpkg-query -W -f='${db:status-flags}' "$1" >/dev/null 2>&1; }

rust_json() { printf '"%s"' "$(printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g')"; }

# 每一筆都寫下「當下實際生效的環境變數」，enter / exit 兩端都能核對
rust_record() {
  mkdir -p "$RUST_LOG_DIR" 2>/dev/null
  printf '{"ts":%s,"mode":%s,"PATH":%s,"LD_LIBRARY_PATH":%s,"CARGO_HOME":%s,"RUSTUP_HOME":%s,"rustc":%s,"cargo":%s,"qemu":%s,"accel":%s}\n' \
    "$(rust_json "$(date -Iseconds)")" "$(rust_json "$1")" \
    "$(rust_json "$PATH")" "$(rust_json "${LD_LIBRARY_PATH:-}")" \
    "$(rust_json "${CARGO_HOME:-}")" "$(rust_json "${RUSTUP_HOME:-}")" \
    "$(rust_json "$(rustc --version 2>/dev/null || echo '')")" \
    "$(rust_json "$(cargo --version 2>/dev/null || echo '')")" \
    "$(rust_json "$(qemu-system-x86_64 --version 2>/dev/null | head -1 || echo '')")" \
    "$(rust_json "${QEMU_ACCEL:-}")" >> "$RUST_ENV_LOG"
}

# ---- KVM 自動偵測 ----
# access() 會算 ACL，所以即使 groups 裡沒有 kvm，只要 /dev/kvm 有 user ACL 也算可用。
detect_accel() {
  local a="${QEMU_ACCEL:-auto}"
  if [ "$a" = auto ]; then
    if [ -e /dev/kvm ] && [ -r /dev/kvm ] && [ -w /dev/kvm ]; then
      a=kvm
    else
      a=tcg
    fi
  fi
  printf '%s\n' "$a"
}

# ---- 依賴閉包：只保留系統未安裝的套件 ----
#   |Depends（備選）與 <virtual> 一律優先選「系統已裝」的提供者，避免把整組拖進來。
pkg_deps() {
  apt-cache depends "$1" 2>/dev/null | awk '
    /^ *\|?Depends: / {
      n=$2; isalt = ($0 ~ /^ *\|/)
      if (n ~ /^</) { gc++; g[gc]="V " n; virt[gc]=gc; alt[gc]=isalt }
      else if (isalt && gc>0 && alt[gc]) g[gc]=g[gc] " " n
      else { gc++; g[gc]="C " n; alt[gc]=isalt }
      next
    }
    /^    / { if (virt[gc]) g[gc]=g[gc] " " $1 }
    END { for (i=1;i<=gc;i++) print g[i] }
  '
}

pkg_resolve() {
  local queue="$*" seen="" cur kind name cand p chosen
  while [ -n "$queue" ]; do
    cur="${queue%% *}"; queue="${queue#* }"; [ "$cur" = "$queue" ] && queue=""
    case " $seen " in *" $cur "*) continue ;; esac
    seen="$seen $cur"
    forced=0
    case " $PKG_FORCE " in *" $cur "*) forced=1 ;; esac
    if [ "$RUST_ALL_DEPS" != 1 ] && [ "$forced" != 1 ] && rust_installed "$cur"; then continue; fi

    while read -r kind name cand; do
      [ -z "$kind" ] && continue
      if [ "$RUST_ALL_DEPS" = 1 ]; then queue="$queue $name"; continue; fi
      chosen=""
      for p in $name $cand; do
        rust_installed "$p" && { chosen="$p"; break; }
      done
      [ -z "$chosen" ] && queue="$queue $name"
    done <<< "$(pkg_deps "$cur")"
  done
  echo $seen
}

# ---- 探索：取得確切 URL / size / SHA512（只在第一次、且只寫 manifest）----
#   apt-get --print-uris 只在「目前目錄沒有該 deb」時才會印 URI，所以要在空目錄跑。
pkg_discover() {
  local root=$1 pkgs p tmp l qurl fn sz hash url mf
  mf="$root/manifest.tsv"
  pkgs="$(pkg_resolve "${@:2}")"
  [ -z "$pkgs" ] && { rust_log "[pkg] 閉包內沒有需要抓的套件"; return 0; }

  tmp="$(mktemp -d)" || return 1
  mkdir -p "$root"
  : > "$mf"
  printf '# pkg\tfilename\turl\tsize\tsha512\n' >> "$mf"

  rust_log "[pkg] 探索 $(echo $pkgs | wc -w) 個套件的確切 URL（apt-get --print-uris）"
  for p in $pkgs; do
    l="$( cd "$tmp" && apt-get --print-uris download "$p" 2>/dev/null | head -1 )"
    [ -z "$l" ] && { rust_log "[pkg]   $p：拿不到 URI，跳過"; continue; }
    # 行格式：'URL' FILENAME SIZE HASH
    read -r qurl fn sz hash <<< "$l"
    # 注意：${qurl#\'} 這種寫法在 bash 會造成引號不平衡（\' 被當成開啟的單引號），
    # 所以用 $'\x27' 先取得單引號字元再當 pattern 用。
    sq=$'\x27'
    url=${qurl#"$sq"}
    url=${url%"$sq"}
    printf '%s\t%s\t%s\t%s\t%s\n' "$p" "$fn" "$url" "$sz" "$hash" >> "$mf"
    rust_log "[pkg]   $p → $url"
  done
  rm -rf "$tmp"
  return 0
}

# ---- 重製：照 manifest 的已知 link 用 curl 下載並驗 SHA512 ----
pkg_fetch() {
  local root=$1 mf pkg fn url sz hash got pkgs
  shift
  mf="$root/manifest.tsv"
  mkdir -p "$root/debs"

  if [ "$RUST_FETCH" = "apt" ]; then
    rust_log "[pkg] RUST_FETCH=apt：走 apt-get download（非重製路徑）"
    pkgs="$(pkg_resolve "$@")"
    ( cd "$root/debs" && apt-get download $pkgs ) || {
      rust_log "[pkg] apt-get download 失敗：確認 /var/lib/apt/lists 有索引（apt update 需要 root）"
      return 1
    }
    return 0
  fi

  if [ ! -f "$mf" ] || [ "$(grep -vc '^#' "$mf" 2>/dev/null)" = 0 ]; then
    rust_log "[pkg] manifest 不存在或沒有資料列 → 先探索"
    pkg_discover "$root" "$@" || return 1
  fi

  rust_log "[pkg] curl 下載（依 $mf）"
  while IFS=$'\t' read -r pkg fn url sz hash; do
    case "$pkg" in ''|'#'*) continue ;; esac
    if [ "$RUST_OFFLINE" = 1 ]; then rust_log "[pkg]   offline: $fn"; continue; fi
    got=""
    if [ -f "$root/debs/$fn" ]; then
      got="SHA512:$(sha512sum "$root/debs/$fn" 2>/dev/null | cut -d' ' -f1)"
    fi
    if [ "$got" = "$hash" ]; then
      rust_log "[pkg]   已存在且 hash 相符：$fn"
      continue
    fi
    rust_log "[pkg]   GET $url"
    curl -fsSL --retry 3 -o "$root/debs/$fn" "$url" || { rust_log "[pkg]   下載失敗：$fn"; return 1; }
    got="SHA512:$(sha512sum "$root/debs/$fn" | cut -d' ' -f1)"
    if [ "$got" != "$hash" ]; then
      rust_log "[pkg]   SHA512 不符：期望 $hash 得到 $got"
      return 1
    fi
    rust_log "[pkg]   SHA512 OK：$fn"
  done < "$mf"
  return 0
}

# ---- 解開 + 建 bin/ ----
pkg_extract() {
  local root=$1 f b tgt
  mkdir -p "$root/root" "$root/bin"
  rust_log "[pkg] 解開 → $root/root"
  for f in "$root/debs"/*.deb; do
    [ -e "$f" ] || continue
    dpkg-deb -x "$f" "$root/root"
  done

  # 同名衝突保留 ELF：Debian 的 cargo 有 Python wrapper（需要 CARGO_PATH）與真二進位同名
  rust_log "[pkg] 建立 $root/bin 符號連結"
  find "$root/root" -type f -perm -u+x -print 2>/dev/null | while read -r f; do
    b="$(basename "$f")"
    if [ -L "$root/bin/$b" ]; then
      tgt="$(readlink "$root/bin/$b")"
      case "$(file -b "$tgt" 2>/dev/null)" in ELF*) continue ;; esac
    fi
    ln -sf "$f" "$root/bin/$b"
  done
  return 0
}
