#!/usr/bin/env bash
# =============================================================================
# setup_env.sh — 進入本地開發環境（要 source）
#
#   source setup_env.sh   → 偵測各 env 是否已安裝，未安裝則觸發 scripts/install_*.sh
#                           然後設定 PATH / LD_LIBRARY_PATH / CARGO_HOME / RUSTUP_HOME，
#                           PS1 加 (rust) 前綴，環境變數寫入 logs/env.jsonl
#   rust_exit             → 離開 env：恢復進入前的設定
#
#   （exit 就是 shell 自己的 exit，不攔；離開 env 用 rust_exit）
#
# 重複 source：先 rust_exit 回到真正的進入前狀態，再重新進入。
# 非 source 執行：開一個互動子 shell，exit 就回到原本 shell。
# =============================================================================

_is_sourced() {
  if [ -n "${BASH_SOURCE+x}" ]; then
    [ "${BASH_SOURCE[0]}" != "$0" ]
  elif [ -n "${(%):-%x}" ]; then          # zsh
    [ "${(%):-%x}" != "$0" ]
  else
    return 1
  fi
}

_self="${BASH_SOURCE[0]:-${(%):-%x}}"
_dir="$(cd "$(dirname "$_self")" 2>/dev/null && pwd || echo .)"
source "$_dir/scripts/common.sh"

# ---- 偵測是否已安裝，未安裝就觸發安裝流程 ----
ensure_installed() {
  local r root
  for r in rust qemu guest llvm; do
    root="$(env_root "$r")"
    if [ ! -f "$root/.install-stamp" ]; then
      rust_log "[env] $r 尚未安裝 → 呼叫 scripts/install_$r.sh"
      "$_dir/scripts/install_$r.sh" || return 1
    fi
  done
  return 0
}

# ---- 環境設定（只設環境，不做安裝）----
env_apply() {
  local libs r root newpath newld

  # 先算好再賦值：如果先把 PATH 清空，下面的 sort / tr 就找不到
  newpath=""
  newld=""
  for r in rust qemu guest llvm; do
    root="$(env_root "$r")"
    [ -d "$root/bin" ] && newpath="${newpath}${root}/bin:"
    newld="${newld}$(find "$root/root" -type f -name '*.so*' -printf '%h\n' 2>/dev/null | sort -u | tr '\n' ':')"
  done
  PATH="${newpath}${ENV_ORIG_PATH}"
  LD_LIBRARY_PATH="${newld}${ENV_ORIG_LD}"

  CARGO_HOME="${ENV_ORIG_CARGO_HOME:-$RUST_ENV_ROOT/cargo-home}"
  RUSTUP_HOME="${ENV_ORIG_RUSTUP_HOME:-$RUST_ENV_ROOT/rustup-home}"
  mkdir -p "$CARGO_HOME" "$RUSTUP_HOME"

  export PATH LD_LIBRARY_PATH CARGO_HOME RUSTUP_HOME
}

env_save() {
  ENV_ORIG_PATH="$PATH"
  ENV_ORIG_LD="${LD_LIBRARY_PATH:-}"
  ENV_ORIG_CARGO_HOME="${CARGO_HOME:-}"
  ENV_ORIG_RUSTUP_HOME="${RUSTUP_HOME:-}"
  ENV_ORIG_PS1="$PS1"
}

rust_exit() {
  if [ "${RUST_ENV_ACTIVE:-0}" != 1 ]; then
    printf '[env] 目前不在 rust env 內（未進入過）\n'
    return 0
  fi
  PATH="$ENV_ORIG_PATH"
  LD_LIBRARY_PATH="$ENV_ORIG_LD"
  CARGO_HOME="$ENV_ORIG_CARGO_HOME"
  RUSTUP_HOME="$ENV_ORIG_RUSTUP_HOME"
  PS1="$ENV_ORIG_PS1"
  RUST_ENV_ACTIVE=0
  rust_record "exit"
  printf '[env] 已離開 rust env（PATH / LD_LIBRARY_PATH / CARGO_HOME / RUSTUP_HOME / PS1 已恢復）\n'
}
env_exit() { rust_exit; }

# ---- 主流程 ----
if ! _is_sourced; then
  ensure_installed || exit 1
  env_save
  env_apply
  rust_record "enter-subshell"
  if [ -t 0 ] && [ -t 1 ]; then
    rc="$(mktemp)"
    { [ -f "$HOME/.bashrc" ] && printf 'source %s\n' "$HOME/.bashrc"; printf 'PS1="(rust) %s\n' "$PS1"; } > "$rc"
    bash --rcfile "$rc"
    rm -f "$rc"
  else
    printf 'export PATH=%s\nexport LD_LIBRARY_PATH=%s\nexport CARGO_HOME=%s\nexport RUSTUP_HOME=%s\n' \
      "$PATH" "$LD_LIBRARY_PATH" "$CARGO_HOME" "$RUSTUP_HOME"
  fi
  exit 0
fi

# 冪等：重複 source 時先離開（回到真正的進入前狀態），再重新進入。
# 否則第二次 source 會把「已被改過的環境」存成 original，rust_exit 就恢復不回原始。
if [ "${RUST_ENV_ACTIVE:-0}" = 1 ]; then
  rust_log "[env] 已在 rust env 內 → 先 rust_exit 再重新進入"
  rust_exit
fi

ensure_installed || return 1 2>/dev/null || exit 1

env_save
env_apply
PS1="(rust) $PS1"
RUST_ENV_ACTIVE=1
rust_record "enter"

if [ "$RUST_QUIET" != 1 ]; then
  printf '[env] %s\n' "$(rustc --version 2>/dev/null || echo 'rustc 不可用')"
  printf '[env] %s\n' "$(cargo --version 2>/dev/null || echo 'cargo 不可用')"
  printf '[env] %s\n' "$(qemu-system-x86_64 --version 2>/dev/null | head -1 || echo 'qemu 不可用')"
  printf '[env] accel=%s（/dev/kvm %s）\n' "$(detect_accel)" "$([ -w /dev/kvm ] && echo '可寫' || echo '不可寫')"
  printf '[env] root=%s\n' "$RUST_ENV_ROOT"
  printf '[env] 已進入 rust env（PS1 加 (rust)）；rust_exit 恢復原本設定\n'
  printf '[env] 環境變數記錄 → %s\n' "$RUST_ENV_LOG"
fi

return 0 2>/dev/null || exit 0
