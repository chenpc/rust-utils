//! id(1) — Print user and group information.
//!
//! 依據 man page（docs/man/id.1.gz）與本機參考（uutils coreutils 0.10.0）實測：
//! - 預設格式 `uid=<uid>(<name>) gid=<gid>(<name>) groups=<gid>(<name>),...`
//!   （man：real 與 effective 不同時兩者都印 → 不同時加 `euid=`/`egid=`）
//! - `-u/-g/-G` 只印 effective ID（`-r` 改用 real）；`-n` 改印名稱
//! - `-z` 用 NUL 取代空白/換行（只與 -u/-g/-G 相容）
//! - `-p` 為 human-readable（每列一項、TAB 分隔）；`-P` 印 passwd 条目
//! - `-A` 在 Linux 是 no-op；`-Z` 一律失敗（無 SELinux/SMACK 支援）
//! - 名稱找不到：預設格式印數字並每筆印一行 warning（rc 仍 0）；`-n` 模式印數字且 rc=1
//! - 名稱查找自行解析 /etc/passwd 與 /etc/group（見 src/passwd.rs）

use crate::passwd::{self, Passwd};
use crate::pathname::infer_long_opt;
use crate::sys;
use crate::{In, Out};

// 逐字取自參考 `id --help`（`  [USER]...  ` 有尾隨兩空白）。
const ID_HELP: &str = r#"Print user and group information for each specified USER,
or (when USER omitted) for the current user.

Usage: id [OPTION]... [USER]...

Arguments:
  [USER]...  

Options:
  -a, --ignore   ignore, for compatibility with other versions
  -A             Display the process audit user ID and other process audit properties,
                 which requires privilege (not available on Linux).
  -u, --user     Display only the effective user ID as a number.
  -g, --group    Display only the effective group ID as a number
  -G, --groups   Display only the different group IDs as white-space separated numbers,
                 in no particular order.
  -n, --name     Display the name of the user or group ID for the -G, -g and -u options
                 instead of the number.
                 If any of the ID numbers cannot be mapped into
                 names, the number will be displayed as usual.
  -P             Display the id as a password file entry.
  -r, --real     Display the real ID for the -G, -g and -u options instead of
                 the effective ID.
  -z, --zero     delimit entries with NUL characters, not whitespace;
                 not permitted in default format
  -Z, --context  print only the security context of the process (not enabled)
  -h, --help     Print help
  -V, --version  Print version

The id utility displays the user and group names and numeric IDs, of the
calling process, to the standard output. If the real and effective IDs are
different, both are displayed, otherwise only the real ID is displayed.

If a user (login name or user ID) is specified, the user and group IDs of
that user are displayed. In this case, the real and effective IDs are
assumed to be the same."#;

const ID_LONGS: &[&str] = &[
    "context",
    "group",
    "groups",
    "help",
    "ignore",
    "name",
    "real",
    "user",
    "version",
    "zero",
];

const ID_USAGE: &str = "Usage: id [OPTION]... [USER]...";

#[derive(Clone, Copy)]
enum Mode {
    User,
    Group,
    Groups,
}

enum Target {
    Current,
    User(Passwd),
}

fn clap_unexpected(arg: &str, stderr: Out) -> i32 {
    writeln!(stderr, "error: unexpected argument '{}' found", arg).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "  tip: to pass '{}' as a value, use '-- {}'", arg, arg).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "{}", ID_USAGE).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "For more information, try '--help'.").ok();
    1
}

fn clap_conflict(first: &str, others: &[&str], stderr: Out) -> i32 {
    if others.len() == 1 {
        writeln!(
            stderr,
            "error: the argument '{}' cannot be used with '{}'",
            first, others[0]
        )
        .ok();
    } else {
        // clap 對多個衝突只印冒號（清單不展開），照參考逐字。
        writeln!(
            stderr,
            "error: the argument '{}' cannot be used with:",
            first
        )
        .ok();
    }
    writeln!(stderr).ok();
    writeln!(stderr, "For more information, try '--help'.").ok();
    1
}

/// clap 的衝突表（實測得出）：顯示名 → 與其衝突的顯示名。
fn conflicts_of(name: &str) -> &'static [&'static str] {
    match name {
        "--user" => &["--group", "--groups", "--context", "-A"],
        "--group" => &["--user", "--groups", "--context", "-A"],
        "--groups" => &["--user", "--group", "--context", "-A"],
        "--context" => &["--user", "--group", "--groups"],
        "-A" => &["--user", "--group", "--groups", "-p", "-P"],
        "-p" => &["-P", "-A"],
        "-P" => &["-p", "-A"],
        _ => &[],
    }
}

/// operand 的引號包裝（實測）：operand 內含 `'` 時改用雙引號，否則單引號。
/// 佐證：`id ""` → `id: '': no such user`；operand 是兩個單引號字元 → `id: "''": no such user`；
/// `id a'b` → `id: "a'b": no such user`。
fn operand_label(op: &str) -> String {
    if op.contains('\'') {
        format!("\"{}\"", op)
    } else {
        format!("'{}'", op)
    }
}

/// 名稱或數字。`use_name` 決定是否解析名稱（`-n` / 預設格式），
/// `warn` 決定解析失敗時是否印 `id: cannot find name for ...`（實測：只有要名稱的
/// 場合才印 warning）。
fn uid_text(uid: u32, use_name: bool, warn: bool, stderr: Out) -> String {
    if !use_name {
        return uid.to_string();
    }
    match passwd::passwd_by_uid(uid) {
        Some(p) => p.name,
        None => {
            if warn {
                writeln!(stderr, "id: cannot find name for user ID {}", uid).ok();
            }
            uid.to_string()
        }
    }
}

fn gid_text(gid: u32, use_name: bool, warn: bool, stderr: Out) -> String {
    if !use_name {
        return gid.to_string();
    }
    match passwd::group_name(gid) {
        Some(n) => n,
        None => {
            if warn {
                writeln!(stderr, "id: cannot find name for group ID {}", gid).ok();
            }
            gid.to_string()
        }
    }
}

/// 目前程序的群組清單：egid 先，再來是 getgroups() 中不等於 egid 的條目
/// （重複的補充群組照參考保留：實測 `1000 65534 65534 ...` 不去重）。
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
    let mut flags: Vec<&'static str> = Vec::new(); // 衝突偵測用（命令列順序）
    let mut mode: Option<Mode> = None;
    let mut human = false;
    let mut passwd_fmt = false;
    let mut names = false;
    let mut real = false;
    let mut zero = false;
    let mut context = false;
    let mut ignore_all = false; // -A（Linux 上無 audit 資訊 → 不印任何東西）
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
            match infer_long_opt(&name, ID_LONGS) {
                Some("help") => {
                    writeln!(stdout, "{}", ID_HELP).ok();
                    return 0;
                }
                Some("version") => {
                    writeln!(stdout, "id (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                Some("ignore") => {}
                Some("context") => {
                    flags.push("--context");
                    context = true;
                }
                Some("user") => {
                    flags.push("--user");
                    mode = Some(Mode::User);
                }
                Some("group") => {
                    flags.push("--group");
                    mode = Some(Mode::Group);
                }
                Some("groups") => {
                    flags.push("--groups");
                    mode = Some(Mode::Groups);
                }
                Some("name") => names = true,
                Some("real") => real = true,
                Some("zero") => zero = true,
                _ => return clap_unexpected(&a, stderr),
            }
            continue;
        }
        let chars: Vec<char> = a[1..].chars().collect();
        let mut k = 0usize;
        let mut bad: Option<String> = None;
        while k < chars.len() {
            match chars[k] {
                'a' => {}
                'A' => {
                    flags.push("-A");
                    ignore_all = true;
                }
                'u' => {
                    flags.push("--user");
                    mode = Some(Mode::User);
                }
                'g' => {
                    flags.push("--group");
                    mode = Some(Mode::Group);
                }
                'G' => {
                    flags.push("--groups");
                    mode = Some(Mode::Groups);
                }
                'p' => {
                    flags.push("-p");
                    human = true;
                }
                'P' => {
                    flags.push("-P");
                    passwd_fmt = true;
                }
                'n' => names = true,
                'r' => real = true,
                'z' => zero = true,
                'Z' => {
                    flags.push("--context");
                    context = true;
                }
                'h' => {
                    writeln!(stdout, "{}", ID_HELP).ok();
                    return 0;
                }
                'V' => {
                    writeln!(stdout, "id (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                _ => {
                    bad = Some(format!("-{}", chars[k]));
                    break;
                }
            }
            k += 1;
        }
        if let Some(b) = bad {
            return clap_unexpected(&b, stderr);
        }
    }

    // clap 層衝突檢查：取命令列中第一個有衝突的旗標。
    for idx in 0..flags.len() {
        let f = flags[idx];
        let set = conflicts_of(f);
        let others: Vec<&str> = flags
            .iter()
            .enumerate()
            .filter(|(j, other)| *j != idx && set.contains(other))
            .map(|(_, other)| *other)
            .collect();
        if !others.is_empty() {
            return clap_conflict(f, &others, stderr);
        }
    }

    // 運行期錯誤的優先序（實測）：-Z → -n/-r 需要 mode → -z 需要 mode。
    if context {
        writeln!(
            stderr,
            "id: --context (-Z) works only on an SELinux/SMACK-enabled kernel"
        )
        .ok();
        return 1;
    }
    if (names || real) && mode.is_none() {
        writeln!(
            stderr,
            "id: printing only names or real IDs requires -u, -g, or -G"
        )
        .ok();
        return 1;
    }
    if zero && mode.is_none() {
        writeln!(stderr, "id: option --zero not permitted in default format").ok();
        return 1;
    }

    // -P：印 passwd 条目（參考對多個 operand 只處理第一個）。
    if passwd_fmt {
        let first = if operands.is_empty() {
            passwd::passwd_by_uid(sys::getuid())
        } else {
            passwd::passwd_by_operand(&operands[0])
        };
        match first {
            Some(p) => {
                writeln!(
                    stdout,
                    "{}:x:{}:{}:{}:{}:{}",
                    p.name, p.uid, p.gid, p.gecos, p.dir, p.shell
                )
                .ok();
                0
            }
            None => {
                if operands.is_empty() {
                    writeln!(stderr, "id: No such id: {}", sys::getuid()).ok();
                } else {
                    writeln!(
                        stderr,
                        "id: {}: no such user",
                        operand_label(&operands[0])
                    )
                    .ok();
                }
                1
            }
        }
    } else if human {
        human_output(&operands, stdout, stderr)
    } else {
        match mode {
            Some(Mode::User) => mode_output(&operands, Mode::User, names, real, zero, stdout, stderr),
            Some(Mode::Group) => {
                mode_output(&operands, Mode::Group, names, real, zero, stdout, stderr)
            }
            Some(Mode::Groups) => {
                mode_output(&operands, Mode::Groups, names, real, zero, stdout, stderr)
            }
            None => {
                if ignore_all {
                    // 參考實測：`id -A`、`id -A root` 都是 rc0 且完全無輸出
                    // （連不存在的 USER 也不報錯）。
                    0
                } else {
                    default_output(&operands, stdout, stderr)
                }
            }
        }
    }
}

fn resolve_operands(operands: &[String], stderr: Out) -> (Vec<Target>, bool) {
    let mut out: Vec<Target> = Vec::new();
    let mut failed = false;
    if operands.is_empty() {
        out.push(Target::Current);
        return (out, false);
    }
    for op in operands {
        match passwd::passwd_by_operand(op) {
            Some(p) => out.push(Target::User(p)),
            None => {
                writeln!(stderr, "id: {}: no such user", operand_label(op)).ok();
                failed = true;
            }
        }
    }
    (out, failed)
}

fn mode_output(
    operands: &[String],
    mode: Mode,
    names: bool,
    real: bool,
    zero: bool,
    stdout: Out,
    stderr: Out,
) -> i32 {
    let (targets, failed) = resolve_operands(operands, stderr);
    if targets.is_empty() {
        return 1;
    }
    let mut rc = if failed { 1 } else { 0 };
    for t in &targets {
        let value = match (t, mode) {
            (Target::Current, Mode::User) => {
                if real {
                    sys::getuid()
                } else {
                    sys::geteuid()
                }
            }
            (Target::Current, Mode::Group) => {
                if real {
                    sys::getgid()
                } else {
                    sys::getegid()
                }
            }
            (Target::User(p), Mode::User) => p.uid,
            (Target::User(p), Mode::Group) => p.gid,
            (_, Mode::Groups) => 0, // 不使用
        };
        let rendered: Vec<String> = match mode {
            Mode::Groups => {
                let list = match t {
                    Target::Current => current_groups(),
                    Target::User(p) => passwd::groups_for(&p.name, p.gid),
                };
                if names && list.iter().any(|g| passwd::group_name(*g).is_none()) {
                    rc = 1;
                }
                list.iter()
                    .map(|g| gid_text(*g, names, names, stderr))
                    .collect()
            }
            Mode::User => {
                let n = if names {
                    if passwd::passwd_by_uid(value).is_none() {
                        rc = 1;
                    }
                    uid_text(value, true, true, stderr)
                } else {
                    value.to_string()
                };
                vec![n]
            }
            Mode::Group => {
                let n = if names {
                    if passwd::group_name(value).is_none() {
                        rc = 1;
                    }
                    gid_text(value, true, true, stderr)
                } else {
                    value.to_string()
                };
                vec![n]
            }
        };
        if zero {
            for r in &rendered {
                write!(stdout, "{}\u{0}", r).ok();
            }
        } else {
            writeln!(stdout, "{}", rendered.join(" ")).ok();
        }
    }
    rc
}

fn human_output(operands: &[String], stdout: Out, stderr: Out) -> i32 {
    if operands.is_empty() {
        let uid = sys::getuid();
        let name = passwd::passwd_by_uid(uid).map(|p| p.name);
        match &name {
            Some(n) => {
                writeln!(stdout, "login\t{}", n).ok();
                writeln!(stdout, "uid\t{}", n).ok();
            }
            None => {
                writeln!(stdout, "uid\t{}", uid).ok();
            }
        }
        let rendered: Vec<String> = current_groups()
            .iter()
            .map(|g| gid_text(*g, true, false, stderr))
            .collect();
        writeln!(stdout, "groups\t{}", rendered.join(" ")).ok();
        return 0;
    }
    let rc = 0;
    for op in operands {
        let p = match passwd::passwd_by_operand(op) {
            Some(p) => p,
            None => {
                writeln!(stderr, "id: {}: no such user", operand_label(op)).ok();
                return 1;
            }
        };
        // 參考怪癖：指定使用者時 `groups<TAB>` 列是空的，群組清單另起一列。
        writeln!(stdout, "uid\t{}", p.name).ok();
        writeln!(stdout, "groups\t").ok();
        let rendered: Vec<String> = passwd::groups_for(&p.name, p.gid)
            .iter()
            .map(|g| gid_text(*g, true, false, stderr))
            .collect();
        writeln!(stdout, "{}", rendered.join(" ")).ok();
    }
    rc
}

fn default_output(operands: &[String], stdout: Out, stderr: Out) -> i32 {
    if operands.is_empty() {
        let uid = sys::getuid();
        let gid = sys::getgid();
        let euid = sys::geteuid();
        let egid = sys::getegid();
        let mut parts: Vec<String> = Vec::new();
        parts.push(format!(
            "uid={}({})",
            uid,
            uid_text(uid, true, true, stderr)
        ));
        if euid != uid {
            parts.push(format!(
                "euid={}({})",
                euid,
                uid_text(euid, true, true, stderr)
            ));
        }
        parts.push(format!("gid={}({})", gid, gid_text(gid, true, true, stderr)));
        if egid != gid {
            parts.push(format!(
                "egid={}({})",
                egid,
                gid_text(egid, true, true, stderr)
            ));
        }
        let rendered: Vec<String> = current_groups()
            .iter()
            .map(|g| format!("{}({})", g, gid_text(*g, true, true, stderr)))
            .collect();
        parts.push(format!("groups={}", rendered.join(",")));
        writeln!(stdout, "{}", parts.join(" ")).ok();
        return 0;
    }
    let mut rc = 0;
    for op in operands {
        let p = match passwd::passwd_by_operand(op) {
            Some(p) => p,
            None => {
                writeln!(stderr, "id: {}: no such user", operand_label(op)).ok();
                rc = 1;
                continue;
            }
        };
        let rendered: Vec<String> = passwd::groups_for(&p.name, p.gid)
            .iter()
            .map(|g| format!("{}({})", g, gid_text(*g, true, true, stderr)))
            .collect();
        writeln!(
            stdout,
            "uid={}({}) gid={}({}) groups={}",
            p.uid,
            p.name,
            p.gid,
            gid_text(p.gid, true, true, stderr),
            rendered.join(",")
        )
        .ok();
    }
    rc
}
