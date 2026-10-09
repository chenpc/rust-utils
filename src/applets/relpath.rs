//! relpath(1) — Print the relative path.
//!
//! GNU coreutils 9.x 新 applet；本機參考實作（uutils 0.10.0）不提供，
//! 行為依 GNU coreutils info（relpath）與 GNU man page 文件實作：
//! - `relpath [-s] [-p] DEST [BASE]`；BASE 預設 `.`
//! - 預設：相對運算元先用 PWD 補成絕對路徑，再做詞法化（去 `.`、多餘 `/`，
//!   `..` 於絕對路徑上直接回退）
//! - `-s|--strict`：兩個運算元都必須是絕對路徑，否則報錯
//! - `-p|--physical`：先解析 symlink（兩邊都要存在）再計算
//! - 輸出「從 BASE 走到 DEST」的相對路徑；同路徑 → `.`
//!
//! 假設記錄於 docs/assumptions.md。

use crate::pathname as p;
use crate::{In, Out};

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut strict = false;
    let mut physical = false;
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
                "strict" => strict = true,
                "physical" => physical = true,
                "help" => {
                    writeln!(stdout, "Usage: relpath [-s] [-p] DEST [BASE]").ok();
                    return 0;
                }
                "version" => {
                    writeln!(stdout, "relpath (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                name => {
                    writeln!(stderr, "relpath: unrecognized option '--{name}'").ok();
                    return 2;
                }
            }
            continue;
        }
        for c in a[1..].chars() {
            match c {
                's' => strict = true,
                'p' => physical = true,
                'h' => {
                    writeln!(stdout, "Usage: relpath [-s] [-p] DEST [BASE]").ok();
                    return 0;
                }
                'V' => {
                    writeln!(stdout, "relpath (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                c => {
                    writeln!(stderr, "relpath: invalid option -- '{c}'").ok();
                    return 2;
                }
            }
        }
    }
    if operands.is_empty() {
        writeln!(stderr, "relpath: missing operand").ok();
        return 1;
    }
    if operands.len() > 2 {
        writeln!(stderr, "relpath: extra operand '{}'", operands[2]).ok();
        return 1;
    }
    let dest = operands[0].clone();
    let base = operands.get(1).cloned().unwrap_or_else(|| ".".to_string());

    if strict {
        if !dest.starts_with('/') || !base.starts_with('/') {
            writeln!(
                stderr,
                "relpath: both paths must be absolute: '{dest}', '{base}'"
            )
            .ok();
            return 1;
        }
    }

    // 非 strict：相對運算元先用 cwd 補成絕對路徑。
    let make_abs = |s: &str| -> Result<String, String> {
        if s.starts_with('/') {
            Ok(s.to_string())
        } else {
            let cwd = std::env::current_dir()
                .map_err(|e| p::io_reason_pub(&e))?
                .to_string_lossy()
                .to_string();
            Ok(format!("{cwd}/{s}"))
        }
    };

    let (dest_abs, base_abs) = if physical {
        let c = p::Canon {
            mode: p::Mode::Existing,
            ..p::Canon::default()
        };
        match (p::resolve_one(&dest, &c), p::resolve_one(&base, &c)) {
            (Ok(d), Ok(b)) => (d, b),
            (Err(e), _) | (_, Err(e)) => {
                writeln!(stderr, "relpath: {e}").ok();
                return 1;
            }
        }
    } else {
        let d = match make_abs(&dest) {
            Ok(v) => v,
            Err(e) => {
                writeln!(stderr, "relpath: {e}").ok();
                return 1;
            }
        };
        let b = match make_abs(&base) {
            Ok(v) => v,
            Err(e) => {
                writeln!(stderr, "relpath: {e}").ok();
                return 1;
            }
        };
        let lex = |s: &str| -> String {
            let comps = p::components(s);
            p::build_lexical_absolute(&comps)
        };
        (lex(&d), lex(&b))
    };

    let out = p::relative_from(&base_abs, &dest_abs);
    writeln!(stdout, "{out}").ok();
    0
}
