//! env(1) — Set each NAME to VALUE in the environment and run COMMAND
//!
//! 依據 man page（docs/man/env.1.gz）的 OPTIONS 清單與 EXTRA：
//! `A mere - implies -i. If no COMMAND, print the resulting environment.`
//! `-f, --file <PATH>  read and set variables from a ".env"-style configuration file (prior to any unset and/or set)`
//!
//! 本專案的兩個刻意決定（記於 docs/assumptions.md）：
//! 1. **不用 exec**：`src/testutil.rs` 的 `t::run` 是 in-process 呼叫 `dispatch`，
//!    若用 `CommandExt::exec()` 會取代掉 test binary。改用 `std::process::Command` spawn + wait
//!    並傳承 exit code（被信号殺 → 128+signal）。
//! 2. **`.env` 文法自訂**：GNU man 未定義文法（參考用 dotenvy，其錯誤措辭如
//!    `8:1 expecting "[Some('='), Some(':')]" but found EOF.` 不予重現）。
//!    本實作：略過空白行與 `#` 註解、以第一個 `=` 切分、兩側 trim、去除成對的 `'…'`/`"…"`、
//!    不展開 `$VAR`、沒有 `=` 的行略過。
//!
//! 參考實測的 exit code 分級：
//! - clap 層錯誤（unknown option、重複旗標、選項缺值）→ 125，並附加 `env: use -[v]S to pass options in shebang lines`
//! - `--null` 與 COMMAND 共存 → 125 + Try 行
//! - `--chdir` 沒有 COMMAND → 125 + Try 行
//! - 無法變更為 DIR → 125 `env: cannot change directory to 'X': <reason (os error N)>`
//! - 信號名不合法 → 125 `env: 'X': invalid signal`（無 Try、無 hint）
//! - 信號無法設定（SIGKILL/SIGSTOP 的 ignore/default）→ 125 `env: failed to set signal action for signal N: Invalid argument`
//! - `.env` 檔讀不到 → 1 `env: <path>: <reason (os error N)>`
//! - COMMAND 找不到 → 127 `env: 'cmd': No such file or directory` + hint
//! - COMMAND 存在但不可執行（如目錄）→ 126 `env: 'cmd': Permission denied`

use crate::pathname::infer_long_opt;
use crate::{In, Out};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;

const ENV_HELP: &str = r#"Set each NAME to VALUE in the environment and run COMMAND

Usage: env [OPTION]... [-] [NAME=VALUE]... [COMMAND [ARG]...]

Arguments:
  [vars]...  

Options:
  -i, --ignore-environment      start with an empty environment
  -C, --chdir <DIR>             change working directory to DIR
  -0, --null                    end each output line with a 0 byte rather than a newline (only valid
                                when printing the environment)
  -f, --file <PATH>             read and set variables from a ".env"-style configuration file (prior
                                to any unset and/or set)
  -u, --unset <NAME>            remove variable from the environment
  -v, --debug...                print verbose information for each processing step
  -S, --split-string <S>        process and split S into separate arguments; used to pass multiple
                                arguments on shebang lines
  -a, --argv0 <a>               Override the zeroth argument passed to the command being executed.
                                Without this option a default value of `command` is used.
      --ignore-signal[=<SIG>]   set handling of SIG signal(s) to do nothing
      --default-signal[=<SIG>]  reset handling of SIG signal(s) to the default action
      --block-signal[=<SIG>]    block delivery of SIG signal(s) while running COMMAND
      --list-signal-handling    list signal handling changes requested by preceding options
  -h, --help                    Print help
  -V, --version                 Print version

A mere - implies -i. If no COMMAND, print the resulting environment."#;

const ENV_USAGE: &str = "env [OPTION]... [-] [NAME=VALUE]... [COMMAND [ARG]...]";
const ENV_LONGS: &[&str] = &[
    "argv0",
    "block-signal",
    "chdir",
    "debug",
    "default-signal",
    "file",
    "help",
    "ignore-environment",
    "ignore-signal",
    "list-signal-handling",
    "null",
    "split-string",
    "unset",
    "version",
];
const SHEBANG_HINT: &str = "env: use -[v]S to pass options in shebang lines";

/// Linux/x86-64 標準信號編號 → 名稱（`--list-signal-handling` 的顯示用）。
const SIG_TABLE: &[(i32, &str)] = &[
    (1, "HUP"),
    (2, "INT"),
    (3, "QUIT"),
    (4, "ILL"),
    (5, "TRAP"),
    (6, "ABRT"),
    (7, "BUS"),
    (8, "FPE"),
    (9, "KILL"),
    (10, "USR1"),
    (11, "SEGV"),
    (12, "USR2"),
    (13, "PIPE"),
    (14, "ALRM"),
    (15, "TERM"),
    (16, "STKFLT"),
    (17, "CHLD"),
    (18, "CONT"),
    (19, "STOP"),
    (20, "TSTP"),
    (21, "TTIN"),
    (22, "TTOU"),
    (23, "URG"),
    (24, "XCPU"),
    (25, "XFSZ"),
    (26, "VTALRM"),
    (27, "PROF"),
    (28, "WINCH"),
    (29, "PWR"),
    (30, "SYS"),
    (34, "RTMIN"),
    (64, "RTMAX"),
];

#[derive(Clone, Copy, PartialEq)]
enum SigAct {
    Ignore,
    Default,
    Block,
}

#[derive(Clone)]
enum Act {
    Set(String, String),
    Unset(String),
}

struct Opts {
    ignore_env: bool,
    chdir: Option<String>,
    nul: bool,
    files: Vec<String>,
    acts: Vec<Act>,
    debug: usize,
    argv0: Option<String>,
    sigs: Vec<(i32, SigAct)>,
    list_sigs: bool,
    command: Option<String>,
    cmd_args: Vec<String>,
}

fn clap_tail(stderr: Out) -> i32 {
    writeln!(stderr).ok();
    writeln!(stderr, "For more information, try '--help'.").ok();
    writeln!(stderr, "{}", SHEBANG_HINT).ok();
    125
}

fn unexpected_arg(a: &str, stderr: Out) -> i32 {
    writeln!(stderr, "error: unexpected argument '{}' found", a).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "  tip: to pass '{}' as a value, use '-- {}'", a, a).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "Usage: {}", ENV_USAGE).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "For more information, try '--help'.").ok();
    writeln!(stderr, "{}", SHEBANG_HINT).ok();
    125
}

fn dup_flag(long: &str, stderr: Out) -> i32 {
    dup_flag_arg(&format!("--{}", long), stderr)
}

/// clap 的重複選項錯誤；`arg` 是含 metavar 的完整顯示名（如 `--chdir <DIR>`）。
fn dup_flag_arg(arg: &str, stderr: Out) -> i32 {
    writeln!(
        stderr,
        "error: the argument '{}' cannot be used multiple times",
        arg
    )
    .ok();
    clap_tail(stderr)
}

fn value_required(long: &str, metavar: &str, stderr: Out) -> i32 {
    crate::pathname::clap_value_required(long, metavar, stderr);
    writeln!(stderr, "{}", SHEBANG_HINT).ok();
    125
}

fn sig_name(sig: i32) -> String {
    SIG_TABLE
        .iter()
        .find(|(n, _)| *n == sig)
        .map(|(_, name)| name.to_string())
        .unwrap_or_else(|| sig.to_string())
}

/// `INT`/`SIGINT`/`int`/`2` 都接受；逗號分隔；空字串視為無信號（不報錯）。
fn parse_signals(spec: &str) -> Result<Vec<i32>, String> {
    let mut out = Vec::new();
    for part in spec.split(',') {
        let t = part.trim();
        if t.is_empty() {
            continue;
        }
        if let Ok(n) = t.parse::<i32>() {
            if (1..=64).contains(&n) {
                out.push(n);
                continue;
            }
            return Err(format!("env: '{}': invalid signal", part));
        }
        let up = t.to_uppercase();
        let name = up.strip_prefix("SIG").unwrap_or(&up);
        match SIG_TABLE.iter().find(|(_, n)| *n == name) {
            Some((sig, _)) => out.push(*sig),
            None => return Err(format!("env: '{}': invalid signal", part)),
        }
    }
    Ok(out)
}

/// `-S` 詞法：空白切分、`'…'`/`"…"` 成對引號吃掉（雙引號內 `\` 可跳脱）、`\` 跳接單字。
fn split_string(s: &str) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut started = false;
    let mut k = 0usize;
    while k < chars.len() {
        let c = chars[k];
        if c == ' ' || c == '\t' || c == '\n' {
            if started {
                out.push(cur.clone());
                cur.clear();
                started = false;
            }
            k += 1;
            continue;
        }
        if c == '\'' {
            started = true;
            k += 1;
            while k < chars.len() && chars[k] != '\'' {
                cur.push(chars[k]);
                k += 1;
            }
            k += 1;
            continue;
        }
        if c == '"' {
            started = true;
            k += 1;
            while k < chars.len() && chars[k] != '"' {
                if chars[k] == '\\' && k + 1 < chars.len() {
                    k += 1;
                }
                cur.push(chars[k]);
                k += 1;
            }
            k += 1;
            continue;
        }
        if c == '\\' && k + 1 < chars.len() {
            started = true;
            k += 1;
            cur.push(chars[k]);
            k += 1;
            continue;
        }
        started = true;
        cur.push(c);
        k += 1;
    }
    if started {
        out.push(cur);
    }
    out
}

fn env_set(table: &mut Vec<(String, String)>, k: &str, v: &str) {
    match table.iter_mut().find(|e| e.0 == k) {
        Some(e) => e.1 = v.to_string(),
        None => table.push((k.to_string(), v.to_string())),
    }
}

fn env_unset(table: &mut Vec<(String, String)>, k: &str) {
    table.retain(|e| e.0 != k);
}

/// 自訂 `.env` 文法（見模組頭部說明）。
fn parse_dotenv(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let (k, v) = match t.split_once('=') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => continue,
        };
        if k.is_empty() {
            continue;
        }
        let val = strip_quotes(v.trim());
        env_set(&mut out, k, &val);
    }
    out
}

/// 去掉值裡成对的单/双引号（`"q w"` → `q w`、`"a"b` → `ab`）；
/// 找不到成对结尾时保留字面引号。不做 `$` 展开。
fn strip_quotes(v: &str) -> String {
    let chars: Vec<char> = v.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '"' || c == '\'' {
            if let Some(close) = (i + 1..chars.len()).find(|&j| chars[j] == c) {
                out.extend(chars[i + 1..close].iter());
                i = close + 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

fn is_assignment(a: &str) -> bool {
    !a.starts_with('-') && a.contains('=')
}

fn exec_bit(path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match std::fs::metadata(path) {
        // 「存在但不是可執行 regular file」= EACCES 類（參考對 `env /etc` 給 126）
        Ok(md) => md.is_file() && md.mode() & 0o111 != 0,
        Err(_) => false,
    }
}

/// 用「修改後」的環境查 PATH；PATH 未設或空 → 內建預設 `/bin:/usr/bin`（參考實測：`env -i echo` 仍成功）。
fn lookup_program(cmd: &str, table: &[(String, String)]) -> Result<String, (i32, bool)> {
    let dirs: Vec<String> = match table.iter().find(|(k, _)| k == "PATH") {
        Some((_, p)) if !p.is_empty() => p.split(':').map(|s| s.to_string()).collect(),
        _ => vec!["/bin".to_string(), "/usr/bin".to_string()],
    };
    let mut saw_non_exec = false;
    if cmd.contains('/') {
        let p = Path::new(cmd);
        if exec_bit(p) {
            return Ok(cmd.to_string());
        }
        if p.exists() {
            saw_non_exec = true;
        }
    } else {
        for d in &dirs {
            let cand = if d.is_empty() {
                Path::new(cmd).to_path_buf()
            } else {
                Path::new(d).join(cmd)
            };
            if exec_bit(&cand) {
                return Ok(cand.to_string_lossy().into_owned());
            }
            if cand.exists() {
                saw_non_exec = true;
            }
        }
    }
    Err(if saw_non_exec { (126, false) } else { (127, true) })
}

extern "C" {
    fn chdir(path: *const i8) -> i32;
    fn signal(sig: i32, disp: usize) -> usize;
    fn sigprocmask(how: i32, set: *const u8, old: *mut u8) -> i32;
}
const SIG_DFL: usize = 0;
const SIG_IGN: usize = 1;
const SIG_ERR: usize = usize::MAX;
const SIG_BLOCK: i32 = 0;

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let raw: Vec<String> = args.to_vec();
    let mut queue: Vec<String> = args.to_vec();
    let mut o = Opts {
        ignore_env: false,
        chdir: None,
        nul: false,
        files: Vec::new(),
        acts: Vec::new(),
        debug: 0,
        argv0: None,
        sigs: Vec::new(),
        list_sigs: false,
        command: None,
        cmd_args: Vec::new(),
    };
    let mut no_more_opts = false;
    let mut saw_assignment = false;
    let mut i = 0usize;

    while i < queue.len() {
        if o.command.is_some() {
            o.cmd_args.push(queue[i].clone());
            i += 1;
            continue;
        }
        let a = queue[i].clone();
        i += 1;
        if no_more_opts {
            o.command = Some(a);
            continue;
        }
        if saw_assignment && !is_assignment(&a) {
            // 第一個 NAME=VALUE 之後：仍是 NAME=VALUE 就繼續當成設定，
            // 第一個「不是設定」的 token（含 `-u`）才是 command（參考實測：
            // `env -i A=1 B=2` → 印兩筆；`env -i -v A=1 -u B true` → executing: '-u'）
            o.command = Some(a);
            continue;
        }
        if a == "--" {
            no_more_opts = true;
            continue;
        }
        if a == "-" {
            if o.ignore_env {
                return dup_flag("ignore-environment", stderr);
            }
            o.ignore_env = true;
            continue;
        }
        if is_assignment(&a) {
            let (k, v) = a.split_once('=').unwrap();
            o.acts.push(Act::Set(k.to_string(), v.to_string()));
            saw_assignment = true;
            continue;
        }
        if a.starts_with("--") {
            let eq = a.find('=');
            let name = a[2..eq.unwrap_or(a.len())].to_string();
            let attached = eq.map(|p| a[p + 1..].to_string());
            let full = match infer_long_opt(&name, ENV_LONGS) {
                Some(f) => f,
                None => return unexpected_arg(&a, stderr),
            };
            match full {
                "help" => {
                    writeln!(stdout, "{}", ENV_HELP).ok();
                    return 0;
                }
                "version" => {
                    writeln!(stdout, "env (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                "ignore-environment" => {
                    if o.ignore_env {
                        return dup_flag("ignore-environment", stderr);
                    }
                    o.ignore_env = true;
                }
                "null" => {
                    if o.nul {
                        return dup_flag("null", stderr);
                    }
                    o.nul = true;
                }
                "list-signal-handling" => {
                    if o.list_sigs {
                        return dup_flag("list-signal-handling", stderr);
                    }
                    o.list_sigs = true;
                }
                "debug" => o.debug += 1,
                "chdir" => {
                    if o.chdir.is_some() {
                        return dup_flag_arg("--chdir <DIR>", stderr);
                    }
                    let v = match attached {
                        Some(v) => v,
                        None => {
                            if i >= queue.len() {
                                return value_required("chdir", "DIR", stderr);
                            }
                            let v = queue[i].clone();
                            i += 1;
                            v
                        }
                    };
                    o.chdir = Some(v);
                }
                "file" => {
                    let v = match attached {
                        Some(v) => v,
                        None => {
                            if i >= queue.len() {
                                return value_required("file", "PATH", stderr);
                            }
                            let v = queue[i].clone();
                            i += 1;
                            v
                        }
                    };
                    o.files.push(v);
                }
                "unset" => {
                    let v = match attached {
                        Some(v) => v,
                        None => {
                            if i >= queue.len() {
                                return value_required("unset", "NAME", stderr);
                            }
                            let v = queue[i].clone();
                            i += 1;
                            v
                        }
                    };
                    o.acts.push(Act::Unset(v));
                }
                "argv0" => {
                    let v = match attached {
                        Some(v) => v,
                        None => {
                            if i >= queue.len() {
                                return value_required("argv0", "a", stderr);
                            }
                            let v = queue[i].clone();
                            i += 1;
                            v
                        }
                    };
                    o.argv0 = Some(v);
                }
                "split-string" => {
                    let v = match attached {
                        Some(v) => v,
                        None => {
                            if i >= queue.len() {
                                return value_required("split-string", "S", stderr);
                            }
                            let v = queue[i].clone();
                            i += 1;
                            v
                        }
                    };
                    let expanded = split_string(&v);
                    queue.splice(i..i, expanded);
                }
                "ignore-signal" | "default-signal" | "block-signal" => {
                    let act = match full {
                        "ignore-signal" => SigAct::Ignore,
                        "default-signal" => SigAct::Default,
                        _ => SigAct::Block,
                    };
                    // 值可省略（`--ignore-signal` 單獨出現時不記任何東西，參考實測）
                    if let Some(spec) = attached {
                        match parse_signals(&spec) {
                            Ok(sigs) => {
                                for s in sigs {
                                    o.sigs.push((s, act));
                                }
                            }
                            Err(msg) => {
                                writeln!(stderr, "{}", msg).ok();
                                return 125;
                            }
                        }
                    }
                }
                _ => return unexpected_arg(&a, stderr),
            }
            continue;
        }
        if a.starts_with('-') && a.len() > 1 {
            let chars: Vec<char> = a[1..].chars().collect();
            let mut k = 0usize;
            while k < chars.len() {
                match chars[k] {
                    'i' => {
                        if o.ignore_env {
                            return dup_flag("ignore-environment", stderr);
                        }
                        o.ignore_env = true;
                    }
                    '0' => {
                        if o.nul {
                            return dup_flag("null", stderr);
                        }
                        o.nul = true;
                    }
                    'v' => o.debug += 1,
                    'C' | 'f' | 'u' | 'a' | 'S' => {
                        if chars[k] == 'C' && o.chdir.is_some() {
                            return dup_flag_arg("--chdir <DIR>", stderr);
                        }
                        let rest: String = chars[k + 1..].iter().collect();
                        let rest = rest.strip_prefix('=').unwrap_or(&rest).to_string();
                        let (long, metavar) = match chars[k] {
                            'C' => ("chdir", "DIR"),
                            'f' => ("file", "PATH"),
                            'u' => ("unset", "NAME"),
                            'a' => ("argv0", "a"),
                            _ => ("split-string", "S"),
                        };
                        let v = if !rest.is_empty() {
                            rest
                        } else {
                            if i >= queue.len() {
                                return value_required(long, metavar, stderr);
                            }
                            let v = queue[i].clone();
                            i += 1;
                            v
                        };
                        k = chars.len();
                        match long {
                            "chdir" => o.chdir = Some(v),
                            "file" => o.files.push(v),
                            "unset" => o.acts.push(Act::Unset(v)),
                            "argv0" => o.argv0 = Some(v),
                            _ => {
                                let expanded = split_string(&v);
                                queue.splice(i..i, expanded);
                            }
                        }
                    }
                    'h' => {
                        writeln!(stdout, "{}", ENV_HELP).ok();
                        return 0;
                    }
                    'V' => {
                        writeln!(stdout, "env (uutils coreutils) 0.10.0").ok();
                        return 0;
                    }
                    c => return unexpected_arg(&format!("-{}", c), stderr),
                }
                k += 1;
            }
            continue;
        }
        // 不startsWith('-') 也没有 '='：就是 COMMAND
        o.command = Some(a);
    }

    // clap 層語意的「選項與 command 衝突」檢查
    if o.nul && o.command.is_some() {
        writeln!(stderr, "env: cannot specify --null (-0) with command").ok();
        writeln!(stderr, "{}", crate::pathname::usage_hint("env")).ok();
        return 125;
    }
    if o.chdir.is_some() && o.command.is_none() {
        writeln!(stderr, "env: must specify command with --chdir (-C)").ok();
        writeln!(stderr, "{}", crate::pathname::usage_hint("env")).ok();
        return 125;
    }

    // 信號請求：依編號去重複（保留第一次的 action）並排序顯示（參考實測為遞升）
    let mut sigs: Vec<(i32, SigAct)> = Vec::new();
    for (s, act) in &o.sigs {
        if sigs.iter().any(|(n, _)| n == s) {
            continue;
        }
        sigs.push((*s, *act));
    }
    sigs.sort_by_key(|(n, _)| *n);
    for (s, act) in &sigs {
        if (*act == SigAct::Ignore || *act == SigAct::Default) && (*s == 9 || *s == 19) {
            writeln!(
                stderr,
                "env: failed to set signal action for signal {}: Invalid argument",
                s
            )
            .ok();
            return 125;
        }
    }
    if o.list_sigs {
        for (s, act) in &sigs {
            let label = match act {
                SigAct::Ignore => "IGNORE",
                SigAct::Default => "DEFAULT",
                SigAct::Block => "BLOCK",
            };
            writeln!(stderr, "{:<11}({}): {}", sig_name(*s), s, label).ok();
        }
    }

    // 組出最終環境：檔內設定 → 命令行 unset/set（man：檔「prior to any unset and/or set」）
    let mut table: Vec<(String, String)> = if o.ignore_env {
        Vec::new()
    } else {
        std::env::vars_os()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.to_string_lossy().into_owned(),
                )
            })
            .collect()
    };
    for f in &o.files {
        match std::fs::read_to_string(f) {
            Ok(text) => {
                for (k, v) in parse_dotenv(&text) {
                    env_set(&mut table, &k, &v);
                }
            }
            Err(e) => {
                writeln!(stderr, "env: {}: {}", f, e).ok();
                return 1;
            }
        }
    }
    for act in &o.acts {
        match act {
            Act::Set(k, v) => env_set(&mut table, k, v),
            Act::Unset(k) => env_unset(&mut table, k),
        }
    }

    if o.debug >= 2 {
        writeln!(stderr, "input args:").ok();
        writeln!(stderr, "arg[0]: 'env'").ok();
        for (n, a) in raw.iter().enumerate() {
            writeln!(stderr, "arg[{}]: '{}'", n + 1, a).ok();
        }
    }

    let Some(cmd) = o.command.clone() else {
        for (k, v) in &table {
            if o.nul {
                write!(stdout, "{}={}\u{0}", k, v).ok();
            } else {
                writeln!(stdout, "{}={}", k, v).ok();
            }
        }
        return 0;
    };

    // chdir 先驗證，措辭與參考一致（帶 "(os error N)"）；實測：`env -v -i -C /tmp/nope /bin/true`
    // 只印 chdir 錯誤、沒有 executing 段 → 驗證必須在 executing 段之前。
    if let Some(dir) = &o.chdir {
        match std::fs::metadata(dir) {
            Ok(md) if md.is_dir() => {}
            Ok(_) => {
                let e = std::io::Error::from_raw_os_error(20);
                writeln!(stderr, "env: cannot change directory to '{}': {}", dir, e).ok();
                return 125;
            }
            Err(e) => {
                writeln!(stderr, "env: cannot change directory to '{}': {}", dir, e).ok();
                return 125;
            }
        }
    }

    if o.debug >= 1 {
        writeln!(stderr, "executing: {}", cmd).ok();
        writeln!(stderr, "   arg[0]= '{}'", cmd).ok();
        for (n, a) in o.cmd_args.iter().enumerate() {
            writeln!(stderr, "   arg[{}]= '{}'", n + 1, a).ok();
        }
    }

    let prog = match lookup_program(&cmd, &table) {
        Ok(p) => p,
        Err((126, _)) => {
            writeln!(stderr, "env: '{}': Permission denied", cmd).ok();
            return 126;
        }
        Err((127, _)) => {
            writeln!(stderr, "env: '{}': No such file or directory", cmd).ok();
            writeln!(stderr, "{}", SHEBANG_HINT).ok();
            return 127;
        }
        Err((c, _)) => return c,
    };

    let mut command = std::process::Command::new(&prog);
    command.args(&o.cmd_args);
    command.env_clear();
    for (k, v) in &table {
        command.env(k, v);
    }
    if let Some(a0) = &o.argv0 {
        command.arg0(a0);
    }
    let chdir_arg = o.chdir.clone();
    let sigs_child = sigs.clone();
    unsafe {
        command.pre_exec(move || {
            if let Some(d) = &chdir_arg {
                if let Ok(c) = std::ffi::CString::new(d.as_bytes()) {
                    if chdir(c.as_ptr()) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
            }
            for (s, act) in &sigs_child {
                match act {
                    SigAct::Ignore => {
                        if signal(*s, SIG_IGN) == SIG_ERR {
                            return Err(std::io::Error::last_os_error());
                        }
                    }
                    SigAct::Default => {
                        if signal(*s, SIG_DFL) == SIG_ERR {
                            return Err(std::io::Error::last_os_error());
                        }
                    }
                    SigAct::Block => {
                        let mut set = [0u8; 128];
                        let word = (*s as usize - 1) / 8;
                        if word < set.len() {
                            set[word] |= 1u8 << ((*s as usize - 1) % 8);
                        }
                        if sigprocmask(SIG_BLOCK, set.as_ptr(), std::ptr::null_mut()) != 0 {
                            return Err(std::io::Error::last_os_error());
                        }
                    }
                }
            }
            Ok(())
        });
    }
    match command.status() {
        Ok(status) => {
            if status.success() {
                return 0;
            }
            match status.code() {
                Some(c) => c,
                None => 128 + status.signal().unwrap_or(0),
            }
        }
        Err(e) => {
            let reason = e.to_string();
            writeln!(stderr, "env: '{}': {}", cmd, reason).ok();
            126
        }
    }
}
