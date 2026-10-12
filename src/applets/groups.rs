//! groups(1) — Print group memberships.
//!
//! 依據 man page（docs/man/groups.1.gz）與本機參考實測：
//! - 無 operand：印目前程序的群組（egid 先、依 getgroups(2)；不去重）
//! - 有 operand：每列 `<operand 原文> : <群組清單>`（man：「If one or more arguments
//!   are specified: if any are user names, the group names are printed」）
//! - 名稱找不到：印數字並每筆印 `groups: cannot find name for group ID <n>`、rc 1
//! - 使用者找不到：`groups: '<op>': no such user`、rc 1
//! - 名稱來源為自行解析的 /etc/passwd 與 /etc/group（見 src/passwd.rs）

use crate::passwd;
use crate::pathname::infer_long_opt;
use crate::sys;
use crate::{In, Out};

// 逐字取自參考 `groups --help`（`data‐base` 用的是 U+2010；`  [USERNAME]...  ` 有尾隨兩空白）。
const GROUPS_HELP: &str = r#"Print group memberships for each USERNAME or, if no USERNAME is specified, for
the current process (which may differ if the groups data‐base has changed).

Usage: groups [OPTION]... [USERNAME]...

Arguments:
  [USERNAME]...  

Options:
  -h, --help     Print help
  -V, --version  Print version"#;

const GROUPS_LONGS: &[&str] = &["help", "version"];

fn clap_unexpected(arg: &str, stderr: Out) -> i32 {
    writeln!(stderr, "error: unexpected argument '{}' found", arg).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "  tip: to pass '{}' as a value, use '-- {}'", arg, arg).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "Usage: groups [OPTION]... [USERNAME]...").ok();
    writeln!(stderr).ok();
    writeln!(stderr, "For more information, try '--help'.").ok();
    1
}

/// 同 id.rs：operand 內含 `'` 時改用雙引號（實測 `groups ""` → `groups: '': no such user`、
/// operand 為兩個單引號字元 → `groups: "''": no such user`）。
fn operand_label(op: &str) -> String {
    if op.contains('\'') {
        format!("\"{}\"", op)
    } else {
        format!("'{}'", op)
    }
}

fn gid_label(gid: u32, stderr: Out) -> (String, bool) {
    match passwd::group_name(gid) {
        Some(n) => (n, true),
        None => {
            writeln!(stderr, "groups: cannot find name for group ID {}", gid).ok();
            (gid.to_string(), false)
        }
    }
}

fn current_groups() -> Vec<u32> {
    let egid = sys::getegid();
    let mut out = vec![egid];
    for g in sys::process_groups() {
        if g != egid {
            out.push(g);
        }
    }
    out
}

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
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
            let name = a[2..].split('=').next().unwrap_or("").to_string();
            match infer_long_opt(&name, GROUPS_LONGS) {
                Some("help") => {
                    writeln!(stdout, "{}", GROUPS_HELP).ok();
                    return 0;
                }
                Some("version") => {
                    writeln!(stdout, "groups (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                _ => return clap_unexpected(a, stderr),
            }
        } else {
            for c in a[1..].chars() {
                match c {
                    'h' => {
                        writeln!(stdout, "{}", GROUPS_HELP).ok();
                        return 0;
                    }
                    'V' => {
                        writeln!(stdout, "groups (uutils coreutils) 0.10.0").ok();
                        return 0;
                    }
                    _ => return clap_unexpected(&format!("-{}", c), stderr),
                }
            }
        }
    }

    let mut rc = 0;
    if operands.is_empty() {
        // 目前程序：只有群組清單，沒有 ` : ` 前綴（參考實測）。
        let mut failed = false;
        let rendered: Vec<String> = current_groups()
            .iter()
            .map(|g| {
                let (label, found) = gid_label(*g, stderr);
                if !found {
                    failed = true;
                }
                label
            })
            .collect();
        writeln!(stdout, "{}", rendered.join(" ")).ok();
        return if failed { 1 } else { 0 };
    }
    for op in &operands {
        let p = match passwd::passwd_by_operand(op) {
            Some(p) => p,
            None => {
                writeln!(stderr, "groups: {}: no such user", operand_label(op)).ok();
                rc = 1;
                continue;
            }
        };
        let rendered: Vec<String> = passwd::groups_for(&p.name, p.gid)
            .iter()
            .map(|g| gid_label(*g, stderr).0)
            .collect();
        writeln!(stdout, "{} : {}", op, rendered.join(" ")).ok();
    }
    rc
}
