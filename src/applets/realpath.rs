//! realpath(1) — Print the resolved file name.
//!
//! 依據 man page（docs/man/realpath.1.gz）+ GNU coreutils 9.5 info：
//! - `-q|--quiet`：不印診斷（失敗時也不印檔名，exit 1）
//! - `-s|--strip|--no-symlinks`：只做詞法化
//! - `-L|--logical`：先處理 `..` 再解析 symlink；`-P|--physical`：預設
//! - `-E|--canonicalize`（預設：除最後一段外都要存在）、`-e|--canonicalize-existing`、`-m|--canonicalize-missing`
//! - `--relative-to=DIR`、`--relative-base=DIR`
//! - `-z|--zero`、`-h`、`-V`
//! - exit 0 當且僅當所有檔名都順利印出

use crate::pathname::{self as p, Canon, Mode};
use crate::{In, Out};

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut quiet = false;
    let mut zero = false;
    let mut mode = Mode::Default;
    let mut strip = false;
    let mut logical = false;
    let mut relative_to: Option<String> = None;
    let mut relative_base: Option<String> = None;
    let mut operands: Vec<String> = Vec::new();
    let mut no_more_opts = false;
    let mut i = 0usize;
    while i < args.len() {
        let a = args[i].clone();
        if no_more_opts || (!a.starts_with('-') || a == "-") {
            operands.push(a);
            i += 1;
            continue;
        }
        if a == "--" {
            no_more_opts = true;
            i += 1;
            continue;
        }
        if a.starts_with("--") {
            let (name, inline) = match a[2..].find('=') {
                Some(k) => (&a[2..2 + k], Some(&a[2 + k + 1..])),
                None => (&a[2..], None),
            };
            let take_value = |i: &mut usize| -> Option<String> {
                if let Some(v) = inline {
                    return Some(v.to_string());
                }
                *i += 1;
                args.get(*i).cloned()
            };
            match name {
                "quiet" => quiet = true,
                "zero" => zero = true,
                "strip" | "no-symlinks" => strip = true,
                "logical" => logical = true,
                "physical" => logical = false,
                "canonicalize" => mode = Mode::Default,
                "canonicalize-existing" => mode = Mode::Existing,
                "canonicalize-missing" => mode = Mode::Missing,
                "relative-to" => match take_value(&mut i) {
                    Some(v) => relative_to = Some(v),
                    None => {
                        writeln!(stderr, "realpath: option requires an argument -- '--relative-to'").ok();
                        return 2;
                    }
                },
                "relative-base" => match take_value(&mut i) {
                    Some(v) => relative_base = Some(v),
                    None => {
                        writeln!(stderr, "realpath: option requires an argument -- '--relative-base'").ok();
                        return 2;
                    }
                },
                "help" => {
                    writeln!(stdout, "Usage: realpath [-q] [-s] [-z] [-L|-P] [-E|-e|-m] [--relative-to=DIR] [--relative-base=DIR] [-h|--help] [-V|--version] <files>").ok();
                    return 0;
                }
                "version" => {
                    writeln!(stdout, "realpath (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                name => {
                    writeln!(stderr, "realpath: unrecognized option '--{name}'").ok();
                    return 2;
                }
            }
            i += 1;
            continue;
        }
        for c in a[1..].chars() {
            match c {
                'q' => quiet = true,
                'z' => zero = true,
                's' => strip = true,
                'L' => logical = true,
                'P' => logical = false,
                'E' => mode = Mode::Default,
                'e' => mode = Mode::Existing,
                'm' => mode = Mode::Missing,
                'h' => {
                    writeln!(stdout, "Usage: realpath [-q] [-s] [-z] [-L|-P] [-E|-e|-m] [--relative-to=DIR] [--relative-base=DIR] [-h|--help] [-V|--version] <files>").ok();
                    return 0;
                }
                'V' => {
                    writeln!(stdout, "realpath (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                c => {
                    writeln!(stderr, "realpath: invalid option -- '{c}'").ok();
                    return 2;
                }
            }
        }
        i += 1;
    }
    if operands.is_empty() {
        writeln!(stderr, "realpath: missing operand").ok();
        return 1;
    }
    let o = Canon {
        mode,
        strip,
        logical,
        relative_to: relative_to.as_deref(),
        relative_base: relative_base.as_deref(),
    };
    let mut status = 0;
    for name in &operands {
        match p::resolve_one(name, &o) {
            Ok(out) => {
                let wrote = if zero {
                    stdout
                        .write_all(out.as_bytes())
                        .and_then(|_| stdout.write_all(&[0u8]))
                } else {
                    writeln!(stdout, "{out}")
                };
                if wrote.is_err() {
                    status = 1;
                }
            }
            Err(reason) => {
                if !quiet {
                    writeln!(stderr, "realpath: {name}: {reason}").ok();
                }
                status = 1;
            }
        }
    }
    status
}
