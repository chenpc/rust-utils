//! mknod(1) — Create special files (block/character devices, FIFOs).
//!
//! 行為依本機參考實作（uutils coreutils 0.10.0）逐 case diff（docs/probes/cluster5.md）：
//! - operands: NAME TYPE[b|c|u|p] [major] [minor]（GNU man：type 四種皆合法）
//! - major/minor 只接受十進位（"invalid digit found in string"）
//! - 給 major 不給 minor → "mknod: Special files require major and minor device numbers."
//! - 預設 mode = 0o666 & !umask；`-m` 精確套用
//! - 非 root 建 b/c 失敗：rc 255、措辭 "mknod: <reason (os error N)>"（參考行為 → assumptions）

use crate::applets::chmod::parse_mode_chmod;
use crate::pathname::io_reason_pub;
use crate::sys;
use crate::{In, Out};

fn current_umask() -> u32 {
    extern "C" {
        fn umask(mask: u32) -> u32;
    }
    let m = unsafe { umask(0) };
    unsafe { umask(m) };
    m
}

fn usage_line() -> &'static str {
    "mknod [OPTION]... NAME TYPE [MAJOR MINOR]"
}

const MKNOD_HELP: &str = r#"Create the special file NAME of the given TYPE.

Usage: mknod [OPTION]... NAME TYPE [MAJOR MINOR]

Arguments:
  <NAME>   name of the new file
  <TYPE>   type of the new file (b, c, u or p)
  [major]  major file type
  [minor]  minor file type

Options:
  -m, --mode <MODE>      set file permission bits to MODE, not a=rw - umask
  -Z                     set SELinux security context of each created directory to the default type
      --context[=<CTX>]  like -Z, or if CTX is specified then set the SELinux or SMACK security
                         context to CTX
  -h, --help             Print help
  -V, --version          Print version

Mandatory arguments to long options are mandatory for short options too.
-m, --mode=MODE set file permission bits to MODE, not a=rw - umask

Both MAJOR and MINOR must be specified when TYPE is b, c, or u, and they
must be omitted when TYPE is p. If MAJOR or MINOR begins with 0x or 0X,
it is interpreted as hexadecimal; otherwise, if it begins with 0, as octal;
otherwise, as decimal. TYPE may be:

- b create a block (buffered) special file
- c, u create a character (unbuffered) special file
- p create a FIFO

NOTE: your shell may have its own version of mknod, which usually supersedes
the version described here. Please refer to your shell's documentation
for details about the options it supports.
"#;

fn unexpected(arg: &str, stderr: Out) -> i32 {
    writeln!(stderr, "error: unexpected argument '{}' found", arg).ok();
    writeln!(stderr).ok();
    if arg.starts_with('-') {
        // clap 只在看起來像選項的 token 提供 tip（實測 `mknod c5 c 1 2 3` 無 tip 行）。
        writeln!(
            stderr,
            "  tip: to pass '{}' as a value, use '-- {}'",
            arg, arg
        )
        .ok();
        writeln!(stderr).ok();
    }
    writeln!(stderr, "Usage: {}", usage_line()).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "For more information, try '--help'.").ok();
    1
}

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut mode: Option<u32> = None;
    let mut operands: Vec<String> = Vec::new();
    let mut no_more_opts = false;
    let mut i = 0usize;
    while i < args.len() {
        let a = args[i].clone();
        i += 1;
        if no_more_opts || (!a.starts_with('-') || a == "-") {
            operands.push(a);
            continue;
        }
        if a == "--" {
            no_more_opts = true;
            continue;
        }
        if a.starts_with("--") {
            let name = a[2..].split('=').next().unwrap_or("").to_string();
            let name = crate::pathname::infer_long_opt(
                &name,
                &["context", "help", "mode", "version"],
            )
            .map(|full| full.to_string())
            .unwrap_or(name);
            match name.as_str() {
                "context" => continue,
                "help" => {
                    write!(stdout, "{}", MKNOD_HELP).ok();
                    return 0;
                }
                "version" => {
                    writeln!(stdout, "mknod (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                "mode" => {
                    let spec = if let Some(pos) = a.find('=') {
                        a[pos + 1..].to_string()
                    } else {
                        if i >= args.len() {
                            crate::pathname::clap_value_required("mode", "MODE", stderr);
                            return 1;
                        }
                        let v = args[i].clone();
                        i += 1;
                        v
                    };
                    match parse_mode_chmod(&spec) {
                        Ok(s) => mode = Some(apply_spec(&s)),
                        Err(e) => {
                            writeln!(stderr, "mknod: invalid mode ({})", e).ok();
                            return 1;
                        }
                    }
                    continue;
                }
                _ => return unexpected(&a, stderr),
            }
        }
        let chars: Vec<char> = a[1..].chars().collect();
        let mut k = 0usize;
        while k < chars.len() {
            match chars[k] {
                'm' => {
                    let rest: String = chars[k + 1..].iter().collect();
                    let rest = rest.strip_prefix('=').unwrap_or(&rest).to_string();
                    let spec = if !rest.is_empty() {
                        rest
                    } else {
                        if i >= args.len() {
                            crate::pathname::clap_value_required("mode", "MODE", stderr);
                            return 1;
                        }
                        let v = args[i].clone();
                        i += 1;
                        v
                    };
                    k = chars.len();
                    match parse_mode_chmod(&spec) {
                        Ok(s) => mode = Some(apply_spec(&s)),
                        Err(e) => {
                            writeln!(stderr, "mknod: invalid mode ({})", e).ok();
                            return 1;
                        }
                    }
                }
                'Z' => {}
                'h' => {
                    write!(stdout, "{}", MKNOD_HELP).ok();
                    return 0;
                }
                'V' => {
                    writeln!(stdout, "mknod (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                c => return unexpected(&format!("-{}", c), stderr),
            }
            k += 1;
        }
    }

    if operands.is_empty() {
        writeln!(stderr, "error: the following required arguments were not provided:").ok();
        writeln!(stderr, "  <NAME>").ok();
        writeln!(stderr, "  <TYPE>").ok();
        writeln!(stderr).ok();
        writeln!(stderr, "Usage: {}", usage_line()).ok();
        writeln!(stderr).ok();
        writeln!(stderr, "For more information, try '--help'.").ok();
        return 1;
    }
    let name = operands[0].clone();
    if operands.len() < 2 {
        writeln!(stderr, "error: the following required arguments were not provided:").ok();
        writeln!(stderr, "  <TYPE>").ok();
        writeln!(stderr).ok();
        writeln!(stderr, "Usage: {}", usage_line()).ok();
        writeln!(stderr).ok();
        writeln!(stderr, "For more information, try '--help'.").ok();
        return 1;
    }
    let ty = operands[1].as_str();
    let (kind, needs_dev) = match ty {
        // Linux S_IF* 型別位元：S_IFBLK=0o060000、S_IFCHR=0o020000、S_IFIFO=0o010000
        "b" => (0o060000u32, true),
        "c" => (0o020000u32, true),
        "u" => (0o020000u32, true),
        "p" => (0o010000u32, false),
        other => {
            writeln!(
                stderr,
                "error: invalid value '{}' for '<TYPE>': invalid device type '{}'",
                other, other
            )
            .ok();
            writeln!(stderr).ok();
            writeln!(stderr, "For more information, try '--help'.").ok();
            return 1;
        }
    };

    let mut dev: u64 = 0;
    // b/c 需要 MAJOR 與 MINOR 兩個值；缺任一（含完全沒給）都是同一條錯誤
    if needs_dev && operands.len() == 2 {
        writeln!(
            stderr,
            "mknod: Special files require major and minor device numbers."
        )
        .ok();
        writeln!(stderr, "{}", crate::pathname::usage_hint("mknod")).ok();
        return 1;
    }
    if operands.len() > 2 {
        // p 類型不該有 MAJOR/MINOR：非數字 → clap [major] 錯；數字 → 明示拒絕
        if !needs_dev {
            match parse_num(&operands[2]) {
                Ok(_) => {
                    writeln!(
                        stderr,
                        "mknod: Fifos do not have major and minor device numbers."
                    )
                    .ok();
                    writeln!(stderr, "{}", crate::pathname::usage_hint("mknod")).ok();
                }
                Err(()) => {
                    writeln!(
                        stderr,
                        "error: invalid value '{}' for '[major]': invalid digit found in string",
                        operands[2]
                    )
                    .ok();
                    writeln!(stderr).ok();
                    writeln!(stderr, "For more information, try '--help'.").ok();
                }
            }
            return 1;
        }
        let extra = &operands[2..];
        if extra.len() == 1 {
            writeln!(
                stderr,
                "mknod: Special files require major and minor device numbers."
            )
            .ok();
            writeln!(stderr, "{}", crate::pathname::usage_hint("mknod")).ok();
            return 1;
        }
        if extra.len() > 2 {
            return unexpected(&extra[2], stderr);
        }
        let major = match parse_num(&extra[0]) {
            Ok(v) => v,
            Err(()) => {
                writeln!(
                    stderr,
                    "error: invalid value '{}' for '[major]': invalid digit found in string",
                    extra[0]
                )
                .ok();
                writeln!(stderr).ok();
                writeln!(stderr, "For more information, try '--help'.").ok();
                return 1;
            }
        };
        let minor = match parse_num(&extra[1]) {
            Ok(v) => v,
            Err(()) => {
                writeln!(
                    stderr,
                    "error: invalid value '{}' for '[minor]': invalid digit found in string",
                    extra[1]
                )
                .ok();
                writeln!(stderr).ok();
                writeln!(stderr, "For more information, try '--help'.").ok();
                return 1;
            }
        };
        dev = makedev(major, minor);
    }

    let perm = mode.unwrap_or(0o666 & !current_umask());
    let r = sys::mknod(&name, kind | perm, dev);
    if r != 0 {
        let e = std::io::Error::last_os_error();
        writeln!(stderr, "{}: {}", crate::pathname::invoked_as("mknod"), io_reason_pub(&e)).ok();
        return 255;
    }
    // mknod(2) 會套用 umask，但參考（uutils 0.10.0）對 `-m` 給的是精確 mode
    // （umask 022 下 `-m 777` → 777、`-m 666` → 666）→ 事後 chmod 補回精確值。
    if mode.is_some() {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = std::fs::set_permissions(&name, std::fs::Permissions::from_mode(perm)) {
            writeln!(
                stderr,
                "{}: {}",
                crate::pathname::invoked_as("mknod"),
                io_reason_pub(&e)
            )
            .ok();
            return 255;
        }
    }
    0
}

/// MAJOR/MINOR：只接受十進位（含前導 0）。GNU info 宣稱 0x=hex / 0=octal，但
/// 本機兩個參考（/usr/bin/mknod 與 uutils 0.10.0）皆對 '0x1' 報
/// "invalid digit found in string"、對 '010'/'09' 照十進位收 → 採參考實測
/// （docs/assumptions.md：GNU info 的 hex/octal 描述未實作）。
fn parse_num(v: &str) -> Result<u64, ()> {
    v.parse::<u64>().map_err(|_| ())
}

fn makedev(major: u64, minor: u64) -> u64 {
    (minor & 0xff) | ((major & 0xfff) << 8) | ((minor & !0xff) << 12) | ((major & !0xfff) << 32)
}

fn apply_spec(spec: &crate::applets::chmod::Spec) -> u32 {
    // 參考實測：symbolic 模式的起始值是 0o666（`-m u+wx` → 766、`-m go=r` → 644），
    // 不是 0；who 省略時的 umask 處理在 chmod 的 parse_mode_chmod 内已完成。
    crate::applets::chmod::apply_spec(0o666, spec)
}
