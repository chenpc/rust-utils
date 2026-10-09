//! basename(1) — Print NAME with any leading directory components removed.
//!
//! 依據 man page（docs/man/basename.1.gz）+ GNU coreutils 9.5 info：
//! - `-a|--multiple`、`-s|--suffix <SUFFIX>`（implies -a）、`-z|--zero`、`-h`、`-V`
//! - 傳統形式 `basename NAME [SUFFIX]`（SUFFIX 含 `/` 時不移除任何東西）
//! - 尾端 `/` 先移除，才做 SUFFIX 比對；NAME 全是 `/` 時輸出 `/`；NAME 為空輸出空字串
//! - NAME 與 SUFFIX 相同時不移除 SUFFIX

use crate::pathname as p;
use crate::{In, Out};

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut multiple = false;
    let mut zero = false;
    let mut suffix: Option<String> = None;
    let mut operands: Vec<String> = Vec::new();
    let mut no_more_opts = false;
    let mut i = 0usize;
    while i < args.len() {
        let a = args[i].clone();
        if !no_more_opts && a == "--" {
            no_more_opts = true;
            i += 1;
            continue;
        }
        if !no_more_opts && a.starts_with('-') && a.len() > 1 {
            if a.starts_with("--") {
                let (name, inline) = match a[2..].find('=') {
                    Some(k) => (&a[2..2 + k], Some(&a[2 + k + 1..])),
                    None => (&a[2..], None),
                };
                match name {
                    "multiple" => multiple = true,
                    "zero" => zero = true,
                    "suffix" => match inline.or_else(|| {
                        i += 1;
                        args.get(i).map(|v| v.as_str())
                    }) {
                        Some(v) => {
                            suffix = Some(v.to_string());
                            // GNU: -s / --suffix 代稱 --multiple。
                            multiple = true;
                        }
                        None => {
                            writeln!(stderr, "basename: option requires an argument -- 's'").ok();
                            return 2;
                        }
                    },
                    "help" => {
                        writeln!(stdout, "Usage: basename [-a] [-s SUFFIX] [-z] [-h|--help] [-V|--version] [name]").ok();
                        return 0;
                    }
                    "version" => {
                        writeln!(stdout, "basename (uutils coreutils) 0.10.0").ok();
                        return 0;
                    }
                    _ => {
                        writeln!(stderr, "basename: unrecognized option '--{name}'").ok();
                        return 2;
                    }
                }
                i += 1;
                continue;
            }
            let chars: Vec<char> = a[1..].chars().collect();
            let mut k = 0usize;
            while k < chars.len() {
                match chars[k] {
                    'a' => multiple = true,
                    'z' => zero = true,
                    's' => {
                        let rest: String = chars[k + 1..].iter().collect();
                        multiple = true;
                        suffix = Some(if !rest.is_empty() {
                            rest
                        } else {
                            i += 1;
                            match args.get(i) {
                                Some(v) => v.clone(),
                                None => {
                                    writeln!(stderr, "basename: option requires an argument -- 's'").ok();
                                    return 2;
                                }
                            }
                        });
                        break;
                    }
                    'h' => {
                        writeln!(stdout, "Usage: basename [-a] [-s SUFFIX] [-z] [-h|--help] [-V|--version] [name]").ok();
                        return 0;
                    }
                    'V' => {
                        writeln!(stdout, "basename (uutils coreutils) 0.10.0").ok();
                        return 0;
                    }
                    c => {
                        writeln!(stderr, "basename: invalid option -- '{c}'").ok();
                        return 2;
                    }
                }
                k += 1;
            }
            i += 1;
            continue;
        }
        operands.push(a);
        i += 1;
    }

    if operands.is_empty() {
        writeln!(stderr, "basename: missing operand").ok();
        return 1;
    }
    if !multiple {
        if operands.len() > 2 {
            writeln!(stderr, "basename: extra operand '{}'", operands[2]).ok();
            return 1;
        }
        if operands.len() == 2 {
            suffix = Some(operands[1].clone());
        }
    }

    let names: &[String] = if multiple { &operands } else { &operands[..1] };
    let mut status = 0;
    for name in names {
        let t = p::strip_trailing_slashes(name);
        let base = if t.is_empty() && !name.is_empty() {
            "/".to_string()
        } else {
            match suffix.as_deref() {
                Some(s) => p::strip_suffix(p::base_part(name), s).to_string(),
                None => p::base_part(name).to_string(),
            }
        };
        let wrote = if zero {
            stdout.write_all(base.as_bytes()).and_then(|_| stdout.write_all(&[0u8]))
        } else {
            writeln!(stdout, "{base}")
        };
        if wrote.is_err() {
            status = 1;
        }
    }
    status
}
