//! readlink(1) — Print value of a symbolic link or canonical file name.
//!
//! 依據 man page（docs/man/readlink.1.gz）+ GNU coreutils 9.5 info：
//! - 預設（readlink 模式）：印出 symlink 的值；不是 symlink 時不輸出、exit 非零
//! - `-f|--canonicalize`（除最後一段外都要存在）、`-e|--canonicalize-existing`、`-m|--canonicalize-missing`
//! - `-n|--no-newline`：只在單一 FILE 時生效（多個 FILE 時警告並照常換行）
//! - `-q|--quiet` / `-s|--silent`：抑制多數錯誤訊息（GNU 預設即抑制）
//! - `-v|--verbose`：回報錯誤訊息
//! - `-z|--zero`、`-h`、`-V`

use std::path::Path;

use crate::pathname::{self as p, Canon, Mode};
use crate::{In, Out};

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut mode: Option<Mode> = None;
    let mut no_newline = false;
    let mut verbose = false;
    let mut zero = false;
    let mut operands: Vec<String> = Vec::new();
    let mut no_more_opts = false;
    for a in args {
        if no_more_opts || (!a.starts_with('-') || a == "-") {
            operands.push(a.clone());
            continue;
        }
        if a == "--" {
            no_more_opts = true;
            continue;
        }
        if a.starts_with("--") {
            match &a[2..] {
                "canonicalize" => mode = Some(Mode::Default),
                "canonicalize-existing" => mode = Some(Mode::Existing),
                "canonicalize-missing" => mode = Some(Mode::Missing),
                "no-newline" => no_newline = true,
                "quiet" | "silent" => verbose = false,
                "verbose" => verbose = true,
                "zero" => zero = true,
                "help" => {
                    writeln!(stdout, "Usage: readlink [-f|-e|-m] [-n] [-q|-s|-v] [-z] [-h|--help] [-V|--version] [files]").ok();
                    return 0;
                }
                "version" => {
                    writeln!(stdout, "readlink (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                name => {
                    writeln!(stderr, "readlink: unrecognized option '--{name}'").ok();
                    return 2;
                }
            }
            continue;
        }
        let mut bad = None;
        for c in a[1..].chars() {
            match c {
                'f' => mode = Some(Mode::Default),
                'e' => mode = Some(Mode::Existing),
                'm' => mode = Some(Mode::Missing),
                'n' => no_newline = true,
                'q' | 's' => verbose = false,
                'v' => verbose = true,
                'z' => zero = true,
                'h' => {
                    writeln!(stdout, "Usage: readlink [-f|-e|-m] [-n] [-q|-s|-v] [-z] [-h|--help] [-V|--version] [files]").ok();
                    return 0;
                }
                'V' => {
                    writeln!(stdout, "readlink (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                c => {
                    bad = Some(c);
                    break;
                }
            }
        }
        if let Some(c) = bad {
            writeln!(stderr, "readlink: invalid option -- '{c}'").ok();
            return 2;
        }
    }
    if operands.is_empty() {
        writeln!(stderr, "readlink: missing operand").ok();
        return 1;
    }
    // -n 只對單一 FILE 有效。
    let newline = if no_newline && operands.len() == 1 {
        false
    } else {
        if no_newline {
            writeln!(stderr, "readlink: ignoring --no-newline with multiple arguments").ok();
        }
        true
    };
    let mut status = 0;
    for name in &operands {
        let out: Result<String, String> = match mode {
            None => std::fs::read_link(name)
                .map(|t| t.to_string_lossy().to_string())
                .or_else(|e| {
                    // 不是 symlink（或讀不到）：依 -v 決定是否回報。
                    // 名稱存在但不是 symlink → EINVAL；不存在 → 直接用 stat 的 io 錯誤。
                    Err(match Path::new(name).symlink_metadata() {
                        Ok(md) if md.file_type().is_symlink() => p::io_reason_pub(&e),
                        Ok(_) => "Invalid argument".to_string(),
                        Err(e2) => p::io_reason_pub(&e2),
                    })
                }),
            Some(m) => p::resolve_one(
                name,
                &Canon {
                    mode: m,
                    ..Canon::default()
                },
            ),
        };
        match out {
            Ok(text) => {
                let wrote = if zero {
                    stdout.write_all(text.as_bytes()).and_then(|_| stdout.write_all(&[0u8]))
                } else if newline {
                    writeln!(stdout, "{text}")
                } else {
                    stdout.write_all(text.as_bytes())
                };
                if wrote.is_err() {
                    status = 1;
                }
            }
            Err(reason) => {
                if verbose {
                    // 空名稱在訊息裡帶引號（與參考實作一致）。
                    let shown = if name.is_empty() { "''".to_string() } else { name.clone() };
                    writeln!(stderr, "readlink: {shown}: {reason}").ok();
                }
                status = 1;
            }
        }
    }
    status
}
