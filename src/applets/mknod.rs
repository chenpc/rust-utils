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

fn unexpected(arg: &str, stderr: Out) -> i32 {
    writeln!(stderr, "error: unexpected argument '{}' found", arg).ok();
    writeln!(stderr).ok();
    writeln!(
        stderr,
        "  tip: to pass '{}' as a value, use '-- {}'",
        arg, arg
    )
    .ok();
    writeln!(stderr).ok();
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
            match name.as_str() {
                "context" => continue,
                "help" => {
                    writeln!(
                        stdout,
                        "Create special files or FIFOs.\n\nUsage: {}",
                        usage_line()
                    )
                    .ok();
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
                    writeln!(
                        stdout,
                        "Create special files or FIFOs.\n\nUsage: {}",
                        usage_line()
                    )
                    .ok();
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
    use crate::applets::chmod::Op;
    let mut perm = 0u32;
    for op in &spec.0 {
        match op {
            Op::Octal(m) => perm = *m,
            Op::Sym(who, oper, perms) => match oper {
                '+' => perm |= who & perms,
                '-' => perm &= !(who & perms),
                _ => perm = (perm & !who) | (who & perms),
            },
        }
    }
    perm
}
