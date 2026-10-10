// ls/dir/vdir：目錄列表。措辭/格式對齊參考 uutils coreutils 0.10.0。
//
// 依據 man page（docs/man/ls.1.gz）＋參考實測（見 docs/assumptions.md ls 節）：
// - 非 tty 預設單欄；-C/-x/dir 多欄（tab-stop 排版，tabsize 預設 8、非 tty 寬 80）
// - 多欄欄寬 = 各欄最大寬+2（末欄不補）；補白時若 tab-stop ≤ 目標先發 \t
// - dir = ls -b -C；vdir = ls -l 但時間欄恆空（uutils quirk）
// - 多 operand：非目錄先（name 排序）、目錄後（排序）；目錄組 "name:" 標題、組間空行；-R 時單 operand 也加標題
// - 錯誤："ls: cannot access 'X': <reason>" rc2；"cannot open directory" rc2
// - total 只出現在目錄內容列表（-d/檔案 operand 列表無 total）
// - 時間：6 個月內 "%b %e %H:%M" 否則 "%b %e %Y"；--time-style=iso/long-iso/full-iso/locale/+FORMAT；off → uutils 拒否措辭
// - -s blocks = st_blocks/2；long 時 blocks 欄在權限前；-k 為 no-op（跟參考實測）
// - -lh/--si：humanize ceil 2 位有效數字（同 dd/du）
// - 引號：預設 literal；-b escape、-Q c-style、-q 控制字元→?
// - 特權位 s/S/t/T；裝置檔 size 欄 = "major, minor"
// - --group-directories-first：目錄（含指向目錄的 symlink）先
// - -f = -a -U；-Z → "? "前綴；-D = dired：long 格式每行 "  " 前綴＋尾行 //DIRED// <name byte offsets>＋//DIRED-OPTIONS// --quoting-style=<S>
//   S 映射（參考實測）：literal/locale/-N/-q→literal；escape(-b)/c(-Q)/clocale→C；shell→shell-show-control；
//   shell-always→shell-always-quote-show-control；shell-escape→shell-escape-show-control；shell-escape-always→shell-escape-always-quote-show-control
// - 引號 shell 系列（參考實測）：shell：含空白/控制→'...'（含真實控制字元）；含 ' → C-style "..."；shell-escape：控制段 $'\n' 分割＋'...'；shell-always(-escape-always)：恒包裹
// - --color[=always|auto|never]：預設 LS_COLORS（di=01;34 ln=01;36 orph=40;31;01 pi=40;33 ex=01;32 …），每色名 `ESC[0m ESC[<code>name ESC[0m`（Q 引號內著色）；auto=仅 tty
// - --hyperlink[=always|auto|never]：OSC8 `ESC]8;;file://<hostname><abspath>ESC\ name ESC]8;;ESC\`；symlink URI=解析後絕對路徑；auto=仅 tty
// - 錯誤措辭：clap 風格（invalid value → rc1＋[possible values: …]＋Usage＋For more information, try '--help'.；unknown → rc2 unexpected argument）；--time-style 未知值不報錯（回退 Default）
// - 不實作：--author 欄位（收參）

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

#[allow(unused_imports)]
use crate::sys;

const S_IFMT: u32 = 0o170000;
const S_IFSOCK: u32 = 0o140000;
const S_IFLNK: u32 = 0o120000;
const S_IFREG: u32 = 0o100000;
const S_IFBLK: u32 = 0o060000;
const S_IFCHR: u32 = 0o020000;
const S_IFDIR: u32 = 0o040000;
const S_IFIFO: u32 = 0o010000;
const S_ISUID: u32 = 0o4000;
const S_ISGID: u32 = 0o2000;
const S_ISVTX: u32 = 0o1000;

#[derive(Clone, Copy, PartialEq)]
enum Format {
    OneCol,
    Columns,
    Rows,
    Long,
    Commas,
    Dired,
}

#[derive(Clone, Copy, PartialEq)]
enum Sort {
    Name,
    Time,
    Size,
    Extension,
    Version,
    None,
}

#[derive(Clone, Copy, PartialEq)]
enum Quote {
    Literal,
    Escape,
    C,
    HideControl,
    Shell,
    ShellAlways,
    ShellEscape,
    ShellEscapeAlways,
}

#[derive(Clone)]
enum TimeStyle {
    Default,
    Iso,
    LongIso,
    FullIso,
    Locale,
    Plus(String),
}

#[derive(Clone)]
struct Opt {
    format: Format,
    sort: Sort,
    reverse: bool,
    all: bool,
    almost: bool,
    dir_mode: bool,
    deref: bool,
    deref_cmdline: bool,
    inode: bool,
    blocks: bool,
    numeric_ids: bool,
    no_group: bool,
    no_owner: bool,
    classify: u8,
    quote: Quote,
    time_field: u8,
    time_style: TimeStyle,
    full_time: bool,
    human: u8,
    width: Option<usize>,
    tabsize: usize,
    zero: bool,
    ignore: Vec<String>,
    group_dirs_first: bool,
    context: bool,
    recursive: bool,
    color: u8, // 0=never 1=auto(tty) 2=always
    hyperlink: u8,
    colors: HashMap<String, String>,
    dired_offsets: RefCell<Vec<(u64, u64)>>,
}

impl Opt {
    fn new(mode: u8) -> Opt {
        let mut o = Opt {
            format: if mode == 2 { Format::Long } else { Format::OneCol },
            sort: Sort::Name,
            reverse: false,
            all: false,
            almost: false,
            dir_mode: false,
            deref: false,
            deref_cmdline: false,
            inode: false,
            blocks: false,
            numeric_ids: false,
            no_group: false,
            no_owner: false,
            classify: 0,
            quote: Quote::Literal,
            time_field: 0,
            time_style: TimeStyle::Default,
            full_time: false,
            human: 0,
            width: None,
            tabsize: 8,
            zero: false,
            ignore: Vec::new(),
            group_dirs_first: false,
            context: false,
            recursive: false,
            color: 0,
            hyperlink: 0,
            colors: default_ls_colors(),
            dired_offsets: RefCell::new(Vec::new()),
        };
        match mode {
            1 => {
                o.format = Format::Columns;
                o.quote = Quote::Escape;
            }
            2 => o.time_style = TimeStyle::Plus(String::new()),
            _ => {}
        }
        o
    }
}

struct Entry {
    name: String,
    path: String,
    md: fs::Metadata,
    link_target: Option<String>,
    link_is_dir: bool,
}

pub fn run_ls(args: &[String], stdin: crate::In, stdout: crate::Out, stderr: crate::Out) -> i32 {
    run_mode(0, args, stdin, stdout, stderr)
}
pub fn run_dir(args: &[String], stdin: crate::In, stdout: crate::Out, stderr: crate::Out) -> i32 {
    run_mode(1, args, stdin, stdout, stderr)
}
pub fn run_vdir(args: &[String], stdin: crate::In, stdout: crate::Out, stderr: crate::Out) -> i32 {
    run_mode(2, args, stdin, stdout, stderr)
}

fn usage(name: &str, stdout: crate::Out) {
    writeln!(stdout, "Usage: {} [OPTION]... [FILE]...", name).ok();
    writeln!(stdout, "List information about the FILEs (the current directory by default).").ok();
    writeln!(stdout, "Sort entries alphabetically under the C locale.").ok();
}


// ----  clap 錯誤 / 顏色 / hyperlink / 錯誤訊息 ----

fn usage_line(name: &str) -> &'static str {
    match name {
        "dir" => "Usage: dir [OPTION]... [FILE]...",
        "vdir" => "Usage: vdir [OPTION]... [FILE]...",
        _ => "Usage: ls [OPTION]... [FILE]...",
    }
}

fn clap_bad_value(name: &str, flag: &str, _placeholder: &str, value: &str, possible: &[&str], stderr: crate::Out) -> i32 {
    // clap invalid-value 措辭（本機參考 uutils 0.10.0 不含 Usage 行）
    let _ = name;
    writeln!(stderr, "error: invalid value '{}' for '{}'", value, flag).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "  [possible values: {}]", possible.join(", ")).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "For more information, try '--help'.").ok();
    1
}

fn clap_value_required(flag: &str, possible: &[&str], stderr: crate::Out) -> i32 {
    // clap 「選項缺值」措辭（參考：error: a value is required for ... rc1）
    writeln!(stderr, "error: a value is required for '{}' but none was supplied", flag).ok();
    if !possible.is_empty() {
        writeln!(stderr).ok();
        writeln!(stderr, "  [possible values: {}]", possible.join(", ")).ok();
    }
    writeln!(stderr).ok();
    writeln!(stderr, "For more information, try '--help'.").ok();
    1
}

fn clap_unexpected(name: &str, arg: &str, stderr: crate::Out) -> i32 {
    writeln!(stderr, "error: unexpected argument '{}' found", arg).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "  tip: to pass '{}' as a value, use '-- {}'", arg, arg).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "{}", usage_line(name)).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "For more information, try '--help'.").ok();
    2
}

fn default_ls_colors() -> HashMap<String, String> {
    let s = "rs=0:di=01;34:ln=01;36:mh=00:pi=40;33:so=01;35:do=01;35:bd=40;33;01:cd=40;33;01:or=40;31;01:mi=00:su=37;41:sg=30;43:ca=00:tw=30;42:ow=34;42:st=37;44:ex=01;32";
    let mut m = HashMap::new();
    for part in s.split(':') {
        if let Some((k, v)) = part.split_once('=') {
            m.insert(k.to_string(), v.to_string());
        }
    }
    m
}

fn ls_colors_env(map: &mut HashMap<String, String>) {
    if let Ok(v) = std::env::var("LS_COLORS") {
        for part in v.split(':') {
            if let Some((k, v2)) = part.split_once('=') {
                map.insert(k.to_string(), v2.to_string());
            }
        }
    }
}

fn color_key(e: &Entry) -> Option<&'static str> {
    let mode = e.md.permissions().mode();
    match mode & S_IFMT {
        S_IFDIR => Some("di"),
        S_IFLNK => match fs::metadata(&e.path) {
            Ok(_) => Some("ln"),
            Err(_) => Some("or"),
        },
        S_IFIFO => Some("pi"),
        S_IFSOCK => Some("so"),
        S_IFBLK => Some("bd"),
        S_IFCHR => Some("cd"),
        S_IFREG => {
            if mode & 0o111 != 0 {
                Some("ex")
            } else {
                None
            }
        }
        _ => None,
    }
}

fn wrap_color(code: &str, display: &str) -> String {
    let mut s = String::new();
    s.push_str("\x1b[0m\x1b[");
    s.push_str(code);
    s.push('m');
    s.push_str(display);
    s.push_str("\x1b[0m");
    s
}

fn hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "localhost".to_string())
}

fn absolute(path: &str) -> String {
    if path.starts_with('/') {
        return path.to_string();
    }
    match std::env::current_dir() {
        Ok(cwd) => {
            let joined = format!("{}/{}", cwd.display(), path);
            let mut stack: Vec<&str> = Vec::new();
            for seg in joined.split('/') {
                match seg {
                    "." | "" => {}
                    ".." => {
                        stack.pop();
                    }
                    s => stack.push(s),
                }
            }
            format!("/{}", stack.join("/"))
        }
        Err(_) => path.to_string(),
    }
}

fn hyperlink_uri(e: &Entry) -> String {
    let target = if (e.md.permissions().mode() & S_IFMT) == S_IFLNK {
        match &e.link_target {
            Some(t) => {
                let dirpart = &e.path[..e.path.len() - e.name.len()];
                absolute(&format!("{}/{}", dirpart.trim_end_matches('/'), t))
            }
            None => absolute(&e.path),
        }
    } else {
        absolute(&e.path)
    };
    format!("file://{}{}", hostname(), target)
}

fn wrap_hyperlink(uri: &str, inner: String) -> String {
    format!("\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\", uri, inner)
}

fn short_err(e: &std::io::Error) -> String {
    let m = e.to_string();
    let m = match m.rfind(" (os error ") {
        Some(i) => m[..i].to_string(),
        None => m,
    };
    match m.find(": ") {
        Some(i) => m[i + 2..].to_string(),
        None => m,
    }
}

fn parse(name: &str, mode: u8, args: &[String], stdout: crate::Out, stderr: crate::Out) -> Result<(Opt, Vec<String>), i32> {
    let mut o = Opt::new(mode);
    let mut operands: Vec<String> = Vec::new();
    let mut only_ops = false;
    let mut i = 0usize;
    while i < args.len() {
        let a = args[i].as_str();
        i += 1;
        if only_ops {
            operands.push(a.to_string());
            continue;
        }
        match a {
            "--" => only_ops = true,
            "--help" => {
                usage(name, stdout);
                return Err(0);
            }
            "-V" | "--version" => {
                writeln!(stdout, "{} (uutils coreutils) 0.10.0", name).ok();
                return Err(0);
            }
            "--format" | "--sort" | "--time" | "--time-style" | "-I" | "--ignore" | "--hide" | "-T" | "--tabsize" | "-w" | "--width" | "--block-size" | "--hyperlink" => {
                // 本機參考（clap）：缺值 → "a value is required for ..." rc1；有值則消耗下一個 arg（即使是 operand）再驗證
                // --hyperlink 為可選值旗標（GNU man SYNOPSIS 無值）：裸用 → always，不消耗 operand
                if a == "--hyperlink" {
                    let nxt = if i < args.len() { args[i].as_str() } else { "" };
                    if matches!(nxt, "always" | "auto" | "never") {
                        i += 1;
                        o.hyperlink = match nxt {
                            "always" => 2,
                            "auto" => 1,
                            _ => 0,
                        };
                    } else {
                        o.hyperlink = 2;
                    }
                    continue;
                }
                if i >= args.len() {
                    let (flag, possible): (&str, &[&str]) = match a {
                        "--format" => ("--format <format>", &["long", "verbose", "single-column", "columns", "vertical", "across", "horizontal", "commas"]),
                        "--sort" => ("--sort <field>", &["name", "none", "time", "size", "version", "extension", "width"]),
                        "--time" => ("--time <field>", &["atime", "ctime", "mtime", "birth"]),
                        "--time-style" => ("--time-style <style>", &[]),
                        "-I" | "--ignore" | "--hide" => ("--ignore <PATTERN>", &[]),
                        "-T" | "--tabsize" => ("--tabsize <TABSIZE>", &[]),
                        "-w" | "--width" => ("--width <COLS>", &[]),
                        "--block-size" => ("--block-size <SIZE>", &[]),
                        _ => ("--hyperlink[=<WHEN>]", &["always", "auto", "never"]),
                    };
                    return Err(clap_value_required(flag, possible, stderr));
                }
                let v = args[i].as_str().to_string();
                i += 1;
                match a {
                    "--format" => set_format(name, &mut o, &v, stderr)?,
                    "--sort" => set_sort(name, &mut o, &v, stderr)?,
                    "--time" => {
                        o.time_field = match v.as_str() {
                            "atime" => 1,
                            "ctime" => 2,
                            "mtime" | "birth" | "creation" => 0,
                            _ => {
                                return Err(clap_bad_value(
                                    name,
                                    "--time <field>",
                                    "",
                                    &v,
                                    &["atime", "ctime", "mtime", "birth"],
                                    stderr,
                                ));
                            }
                        };
                    }
                    "--time-style" => {
                        o.time_style = match v.as_str() {
                            "full-iso" | "posix-full-iso" => TimeStyle::FullIso,
                            "long-iso" | "posix-long-iso" => TimeStyle::LongIso,
                            "iso" | "posix-iso" => TimeStyle::Iso,
                            "locale" | "posix-locale" => TimeStyle::Locale,
                            s if s.starts_with('+') => TimeStyle::Plus(s[1..].to_string()),
                            _ => TimeStyle::Default,
                        };
                    }
                    "-I" | "--ignore" | "--hide" => o.ignore.push(v),
                    "--hyperlink" => {
                        o.hyperlink = match v.as_str() {
                            "always" => 2,
                            "auto" => 1,
                            "never" => 0,
                            _ => {
                                return Err(clap_bad_value(
                                    name,
                                    "--hyperlink[=<WHEN>]",
                                    "",
                                    &v,
                                    &["always", "auto", "never"],
                                    stderr,
                                ));
                            }
                        };
                    }
                    "-T" | "--tabsize" => {
                        o.tabsize = v.parse().map_err(|_| {
                            writeln!(stderr, "{}: invalid tab size: '{}'", name, v).ok();
                            2
                        })?;
                        if o.tabsize == 0 {
                            o.tabsize = usize::MAX;
                        }
                    }
                    "-w" | "--width" => {
                        o.width = Some(v.parse().map_err(|_| {
                            writeln!(stderr, "{}: invalid line width: '{}'", name, v).ok();
                            2
                        })?);
                    }
                    "--block-size" => {
                        // 參考：接受 GNU size 語法（數字與/或單位字母），否則 rc2
                        let ok = !v.is_empty()
                            && v.chars().all(|c| c.is_ascii_digit() || c.is_ascii_alphabetic())
                            && (v.chars().any(|c| c.is_ascii_digit()) || v.chars().all(|c| c.is_ascii_alphabetic()));
                        if !ok {
                            writeln!(stderr, "{}: invalid --block-size argument '{}'", name, v).ok();
                            return Err(2);
                        }
                    }
                    _ => unreachable!(),
                }
            }
            s if s.starts_with("--time-style=") => {
                let v = s["--time-style=".len()..].to_string();
                o.time_style = match v.as_str() {
                    "full-iso" | "posix-full-iso" => TimeStyle::FullIso,
                    "long-iso" | "posix-long-iso" => TimeStyle::LongIso,
                    "iso" | "posix-iso" => TimeStyle::Iso,
                    "locale" | "posix-locale" => TimeStyle::Locale,
                    _ if v.starts_with('+') => TimeStyle::Plus(v[1..].to_string()),
                    _ => TimeStyle::Default,
                };
            }
            s if s.starts_with("--format=") => set_format(name, &mut o, &s["--format=".len()..], stderr)?,
            s if s.starts_with("--sort=") => set_sort(name, &mut o, &s["--sort=".len()..], stderr)?,
            s if s.starts_with("--time=") => {
                o.time_field = match &s["--time=".len()..] {
                    "atime" | "access" | "use" => 1,
                    "ctime" | "status" => 2,
                    "mtime" | "modification" | "birth" | "creation" => 0,
                    v => {
                        return Err(clap_bad_value(
                            name,
                            "--time <field>",
                            "",
                            v,
                            &["atime", "ctime", "mtime", "birth"],
                            stderr,
                        ));
                    }
                }
            }
            s if s.starts_with("--quoting-style") => {
                let v = match s.split_once('=') {
                    Some((_, v)) => v.to_string(),
                    None => {
                        if i >= args.len() {
                            writeln!(stderr, "error: a value is required for '--quoting-style <quoting-style>' but none was supplied").ok();
                            writeln!(stderr).ok();
                            writeln!(stderr, "For more information, try '--help'.").ok();
                            return Err(2);
                        }
                        let v = args[i].clone();
                        i += 1;
                        v
                    }
                };
                o.quote = match v.as_str() {
                    "literal" | "locale" => Quote::Literal,
                    "clocale" | "c" => Quote::C,
                    "escape" => Quote::Escape,
                    "shell" => Quote::Shell,
                    "shell-always" => Quote::ShellAlways,
                    "shell-escape" => Quote::ShellEscape,
                    "shell-escape-always" => Quote::ShellEscapeAlways,
                    _ => {
                        return Err(clap_bad_value(
                            name,
                            "--quoting-style <quoting-style>",
                            "",
                            &v,
                            &["literal", "locale", "shell", "shell-always", "shell-escape", "shell-escape-always", "clocale", "c", "escape"],
                            stderr,
                        ));
                    }
                };
            }
            s if s.starts_with("--indicator-style") => {
                let v = match s.split_once('=') {
                    Some((_, v)) => v.to_string(),
                    None => {
                        if i >= args.len() {
                            writeln!(stderr, "error: a value is required for '--indicator-style <indicator-style>' but none was supplied").ok();
                            writeln!(stderr).ok();
                            writeln!(stderr, "For more information, try '--help'.").ok();
                            return Err(2);
                        }
                        let v = args[i].clone();
                        i += 1;
                        v
                    }
                };
                o.classify = match v.as_str() {
                    "none" => 0,
                    "slash" => 3,
                    "file-type" => 2,
                    "classify" => 1,
                    _ => {
                        return Err(clap_bad_value(
                            name,
                            "--indicator-style <indicator-style>",
                            "",
                            &v,
                            &["none", "slash", "file-type", "classify"],
                            stderr,
                        ));
                    }
                };
            }
            s if s.starts_with("--ignore=") => o.ignore.push(s["--ignore=".len()..].to_string()),
            s if s.starts_with("--hide=") => o.ignore.push(s["--hide=".len()..].to_string()),
            s if s.starts_with("--color") => {
                let v = match s.split_once('=') {
                    Some((_, v)) => v,
                    None => "always",
                };
                o.color = match v {
                    "always" => 2,
                    "auto" => 1,
                    "never" => 0,
                    _ => {
                        return Err(clap_bad_value(name, "--color[=<WHEN>]", "", v, &["always", "auto", "never"], stderr));
                    }
                };
                ls_colors_env(&mut o.colors);
            }
            s if s.starts_with("--hyperlink") => {
                let (v, given) = match s.split_once('=') {
                    Some((_, v)) => (v, true),
                    None => ("always", false),
                };
                o.hyperlink = match v {
                    "always" => 2,
                    "auto" => 1,
                    "never" => 0,
                    _ => {
                        if given {
                            return Err(clap_bad_value(
                                name,
                                "--hyperlink[=<WHEN>]",
                                "",
                                v,
                                &["always", "auto", "never"],
                                stderr,
                            ));
                        }
                        2
                    }
                };
            }
            // 參考（clap）接受 equals 形式的數值選項
            s if s.starts_with("--width=") => {
                let v = &s["--width=".len()..];
                o.width = Some(v.parse().map_err(|_| {
                    writeln!(stderr, "{}: invalid line width: '{}'", name, v).ok();
                    2
                })?);
            }
            s if s.starts_with("--tabsize=") => {
                let v = &s["--tabsize=".len()..];
                o.tabsize = v.parse().map_err(|_| {
                    writeln!(stderr, "{}: invalid tab size: '{}'", name, v).ok();
                    2
                })?;
                if o.tabsize == 0 {
                    o.tabsize = usize::MAX;
                }
            }
            s if s.starts_with("--block-size=") => {
                let v = &s["--block-size=".len()..];
                let ok = !v.is_empty()
                    && v.chars().all(|c| c.is_ascii_digit() || c.is_ascii_alphabetic())
                    && (v.chars().any(|c| c.is_ascii_digit()) || v.chars().all(|c| c.is_ascii_alphabetic()));
                if !ok {
                    writeln!(stderr, "{}: invalid --block-size argument '{}'", name, v).ok();
                    return Err(2);
                }
            }
            s if s.starts_with("--") => match s {
                "--all" => o.all = true,
                "--almost-all" => o.almost = true,
                "--directory" => o.dir_mode = true,
                "--dereference" => o.deref = true,
                "--dereference-command-line" | "--dereference-command-line-symlink-to-dir" => o.deref_cmdline = true,
                "--inode" => o.inode = true,
                "--size" => o.blocks = true,
                "--numeric-uid-gid" => o.numeric_ids = true,
                "--no-group" => o.no_group = true,
                "--hide-control-chars" => o.quote = Quote::HideControl,
                "--show-control-chars" => o.quote = Quote::Literal,
                "--escape" => o.quote = Quote::Escape,
                "--literal" => o.quote = Quote::Literal,
                "--quote-name" => o.quote = Quote::C,
                "--recursive" => o.recursive = true,
                "--human-readable" => o.human = 1,
                "--kibibytes" => {}
                "--si" => o.human = 2,
                "--full-time" => o.full_time = true,
                "--reverse" => o.reverse = true,
                "--long" => o.format = Format::Long,
                "--zero" => o.zero = true,
                "--context" => o.context = true,
                "--author" => {}
                "--hyperlink" => o.hyperlink = 2,
                "--group-directories-first" => o.group_dirs_first = true,
                "--classify" => {
                    if o.classify == 0 {
                        o.classify = 1;
                    }
                }
                "--file-type" => {
                    if o.classify == 0 || o.classify == 3 {
                        o.classify = 2;
                    }
                },
                "--dired" => o.format = Format::Dired,
                "--single-character" => {}
                "--absolute-paths" => return Err(clap_unexpected(name, "--absolute-paths", stderr)),
                _ => return Err(clap_unexpected(name, s, stderr)),
            },
            s if s.starts_with('-') && s.len() > 1 => {
                let mut ci = 1usize;
                while ci < s.len() {
                    let c = s.as_bytes()[ci] as char;
                    if c == 'T' || c == 'w' {
                        let rest = &s[ci + 1..];
                        let v = if !rest.is_empty() {
                            rest.to_string()
                        } else if i < args.len() {
                            let v = args[i].clone();
                            i += 1;
                            v
                        } else {
                            writeln!(stderr, "{}: option requires an argument -- '{}'", name, c).ok();
                            return Err(2);
                        };
                        if c == 'T' {
                            o.tabsize = v.parse().map_err(|_| {
                                writeln!(stderr, "{}: invalid tab size: '{}'", name, v).ok();
                                2
                            })?;
                            if o.tabsize == 0 {
                                o.tabsize = usize::MAX;
                            }
                        } else {
                            o.width = Some(v.parse().map_err(|_| {
                                writeln!(stderr, "{}: invalid line width: '{}'", name, v).ok();
                                2
                            })?);
                        }
                        break;
                    }
                    match c {
                        'a' => o.all = true,
                        'A' => o.almost = true,
                        'd' => o.dir_mode = true,
                        'L' => o.deref = true,
                        'H' => o.deref_cmdline = true,
                        'i' => o.inode = true,
                        's' => o.blocks = true,
                        'n' => {
                            o.numeric_ids = true;
                            o.format = Format::Long;
                        }
                        'o' | 'G' => {
                            o.no_group = true;
                            o.format = Format::Long;
                        }
                        'g' => {
                            o.no_owner = true;
                            o.format = Format::Long;
                        }
                        'b' => o.quote = Quote::Escape,
                        'N' => o.quote = Quote::Literal,
                        'Q' => o.quote = Quote::C,
                        'q' => o.quote = Quote::HideControl,
                        'R' => o.recursive = true,
                        'r' => o.reverse = true,
                        'h' => o.human = 1,
                        'k' => {}
                        'F' => o.classify = 1,
                        'p' => {
                            if o.classify == 0 {
                                o.classify = 3;
                            }
                        },
                        'Z' => o.context = true,
                        'D' => o.format = Format::Dired,
                        '1' => o.format = Format::OneCol,
                        'C' => o.format = Format::Columns,
                        'x' => o.format = Format::Rows,
                        'm' => o.format = Format::Commas,
                        'l' => o.format = Format::Long,
                        'S' => o.sort = Sort::Size,
                        't' => o.sort = Sort::Time,
                        'v' => o.sort = Sort::Version,
                        'X' => o.sort = Sort::Extension,
                        'U' => o.sort = Sort::None,
                        'f' => {
                            o.all = true;
                            o.sort = Sort::None;
                        }
                        'c' => o.time_field = 2,
                        'u' => o.time_field = 1,
                        'B' => o.ignore.push("*~".to_string()),
                        _ => {
                            writeln!(stderr, "{}: invalid option -- '{}'", name, c).ok();
                            writeln!(stderr, "For a full summary of options, run '{} --help'.", name).ok();
                            return Err(2);
                        }
                    }
                    ci += 1;
                }
            }
            _ => operands.push(a.to_string()),
        }
    }
    Ok((o, operands))
}

fn set_format(name: &str, o: &mut Opt, v: &str, stderr: crate::Out) -> Result<(), i32> {
    o.format = match v {
        "columns" | "vertical" => Format::Columns,
        "single-column" => Format::OneCol,
        "across" => Format::Rows,
        "horizontal" => Format::Columns,
        "long" | "verbose" => Format::Long,
        "commas" => Format::Commas,
        // GNU man：--format 亦接受 time-style 名稱（iso/long-iso/full-iso/verbatim）
        "iso" => {
            o.time_style = TimeStyle::Iso;
            Format::Long
        }
        "long-iso" => {
            o.time_style = TimeStyle::LongIso;
            Format::Long
        }
        "full-iso" => {
            o.time_style = TimeStyle::FullIso;
            Format::Long
        }
        "verbatim" => o.format,
        _ => {
            return Err(clap_bad_value(
                name,
                "--format <format>",
                "",
                v,
                &[
                    "long",
                    "verbose",
                    "single-column",
                    "columns",
                    "vertical",
                    "across",
                    "horizontal",
                    "commas",
                ],
                stderr,
            ));
        }
    };
    Ok(())
}

fn set_sort(name: &str, o: &mut Opt, v: &str, stderr: crate::Out) -> Result<(), i32> {
    o.sort = match v {
        "none" => Sort::None,
        "time" => Sort::Time,
        "size" => Sort::Size,
        "version" => Sort::Version,
        "extension" => Sort::Extension,
        "name" => Sort::Name,
        "width" => Sort::Name,
        _ => {
            return Err(clap_bad_value(
                name,
                "--sort <field>",
                "",
                v,
                &["name", "none", "time", "size", "version", "extension", "width"],
                stderr,
            ));
        }
    };
    Ok(())
}

// ---- 時間 ----

fn time_of(md: &fs::Metadata, field: u8) -> (i64, u64) {
    match field {
        1 => (md.atime(), md.atime_nsec() as u64),
        2 => (md.ctime(), md.ctime_nsec() as u64),
        _ => (md.mtime(), md.mtime_nsec() as u64),
    }
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const WDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

fn civil(sec: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = sec.div_euclid(86400);
    let rem = sec.rem_euclid(86400);
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d as u32, (rem / 3600) as u32, (rem / 60 % 60) as u32, (rem % 60) as u32)
}

fn local_tz(sec: i64) -> i64 {
    crate::sys::local_tm(sec).map(|tm| tm.tm_gmtoff).unwrap_or(0)
}

fn fmt_time(sec: i64, nsec: u64, style: &TimeStyle, full: bool) -> String {
    if full {
        return fmt_time(sec, nsec, &TimeStyle::FullIso, false);
    }
    let tz = local_tz(sec);
    let (y, mo, d, h, mi, s) = civil(sec + tz);
    match style {
        TimeStyle::Plus(f) => {
            if f.is_empty() {
                return String::new();
            }
            datefmt(f, sec + tz, tz)
        }
        TimeStyle::Default | TimeStyle::Locale => {
            let now = unsafe {
                extern "C" {
                    fn time(t: *const i64) -> i64;
                }
                time(std::ptr::null())
            };
            let tm = crate::sys::local_tm(sec).unwrap_or_default();
            if (sec - now).abs() <= 183 * 86400 {
                format!("{} {:>2} {:02}:{:02}", MONTHS[(tm.tm_mon as usize) % 12], tm.tm_mday, tm.tm_hour, tm.tm_min)
            } else {
                format!("{} {:>2}  {}", MONTHS[(tm.tm_mon as usize) % 12], tm.tm_mday, tm.tm_year + 1900)
            }
        }
        TimeStyle::Iso => {
            let now = unsafe {
                extern "C" {
                    fn time(t: *const i64) -> i64;
                }
                time(std::ptr::null())
            };
            // 參考 quirk：old（>6 個月）採 ISO 日期＋尾空格。
            if (sec - now).abs() <= 183 * 86400 {
                format!("{:02}-{:02} {:02}:{:02}", mo, d, h, mi)
            } else {
                format!("{:04}-{:02}-{:02} ", y, mo, d)
            }
        }
        TimeStyle::LongIso => format!("{:04}-{:02}-{:02} {:02}:{:02}", y, mo, d, h, mi),
        TimeStyle::FullIso => {
            let sign = if tz < 0 { '-' } else { '+' };
            let ao = tz.abs();
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:09} {}{:02}{:02}",
                y, mo, d, h, mi, s, nsec, sign, ao / 3600, ao % 3600 / 60
            )
        }
    }
}

fn datefmt(f: &str, lsec: i64, tz: i64) -> String {
    let (y, mo, d, h, mi, s) = civil(lsec);
    let mut res = String::new();
    let mut it = f.chars().peekable();
    while let Some(c) = it.next() {
        if c == '%' {
            if let Some(p) = it.next() {
                match p {
                    'Y' => res.push_str(&format!("{:04}", y)),
                    'm' => res.push_str(&format!("{:02}", mo)),
                    'd' => res.push_str(&format!("{:02}", d)),
                    'H' => res.push_str(&format!("{:02}", h)),
                    'M' => res.push_str(&format!("{:02}", mi)),
                    'S' => res.push_str(&format!("{:02}", s)),
                    'b' => res.push_str(MONTHS[(mo - 1) as usize]),
                    'a' => res.push_str(WDAYS[(lsec.div_euclid(86400).rem_euclid(7)) as usize]),
                    'F' => res.push_str(&format!("{:04}-{:02}-{:02}", y, mo, d)),
                    'T' => res.push_str(&format!("{:02}:{:02}:{:02}", h, mi, s)),
                    'Z' => {
                        let sign = if tz < 0 { '-' } else { '+' };
                        let ao = tz.abs();
                        res.push_str(&format!("{}{:02}{:02}", sign, ao / 3600, ao % 3600 / 60))
                    }
                    '%' => res.push('%'),
                    _ => {
                        res.push('%');
                        res.push(p);
                    }
                }
                continue;
            }
        }
        res.push(c);
    }
    res
}

// ---- 名稱/權限/排版 ----

fn quote_name(name: &str, o: &Opt) -> String {
    match o.quote {
        Quote::Literal => name.to_string(),
        Quote::HideControl => name
            .chars()
            .map(|c| if (c as u32) < 0x20 || c as u32 == 0x7f { '?' } else { c })
            .collect(),
        Quote::Escape => escape_c(name, false),
        Quote::C => escape_c(name, true),
        Quote::Shell | Quote::ShellAlways | Quote::ShellEscape | Quote::ShellEscapeAlways => shell_quoted(name, o.quote),
    }
}

// tty 判定：dyn Write 背後無法可靠辨識真實終端（guest serial console 是 tty 但
// in-process 測試寫入 Vec）→ auto 一律視為非 tty（hyperlink/color 的 auto 等同
// never）。--hyperlink=always / --color=always 仍強制啟用。記 assumptions。
fn stream_isatty(out: &mut dyn Write) -> bool {
    let _ = out;
    false
}

fn hyperlink_active(o: &Opt, out: &mut dyn Write) -> bool {
    o.hyperlink == 2 || (o.hyperlink == 1 && stream_isatty(out))
}

fn color_active(o: &Opt, out: &mut dyn Write) -> bool {
    o.color == 2 || (o.color == 1 && stream_isatty(out))
}

// 寫入 entry 名稱（含引號/顏色/hyperlink），並記錄 dired 名稱 byte 偏移
fn style_name(q: Quote) -> &'static str {
    match q {
        Quote::Literal | Quote::HideControl => "literal",
        Quote::Escape | Quote::C => "C",
        Quote::Shell => "shell-show-control",
        Quote::ShellAlways => "shell-always-quote-show-control",
        Quote::ShellEscape => "shell-escape-show-control",
        Quote::ShellEscapeAlways => "shell-escape-always-quote-show-control",
    }
}

fn escape_c(name: &str, quotes: bool) -> String {
    let mut out = String::new();
    if quotes {
        out.push('"');
    }
    for c in name.chars() {
        match c {
            '"' if quotes => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ' ' if !quotes => out.push_str("\\ "),
            '\0' => out.push_str("\\0"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => out.push_str(&format!("\\{:03o}", c as u32)),
            c => out.push(c),
        }
    }
    if quotes {
        out.push('"');
    }
    out
}

fn mode_string(md: &fs::Metadata) -> String {
    let mode = md.permissions().mode();
    let t = match mode & S_IFMT {
        S_IFDIR => 'd',
        S_IFLNK => 'l',
        S_IFCHR => 'c',
        S_IFBLK => 'b',
        S_IFIFO => 'p',
        S_IFSOCK => 's',
        _ => '-',
    };
    let mut b: [char; 9] = [
        if mode & 0o400 != 0 { 'r' } else { '-' },
        if mode & 0o200 != 0 { 'w' } else { '-' },
        if mode & 0o100 != 0 { 'x' } else { '-' },
        if mode & 0o040 != 0 { 'r' } else { '-' },
        if mode & 0o020 != 0 { 'w' } else { '-' },
        if mode & 0o010 != 0 { 'x' } else { '-' },
        if mode & 0o004 != 0 { 'r' } else { '-' },
        if mode & 0o002 != 0 { 'w' } else { '-' },
        if mode & 0o001 != 0 { 'x' } else { '-' },
    ];
    if mode & S_ISUID != 0 {
        b[2] = if b[2] == 'x' { 's' } else { 'S' };
    }
    if mode & S_ISGID != 0 {
        b[5] = if b[5] == 'x' { 's' } else { 'S' };
    }
    if mode & S_ISVTX != 0 {
        b[8] = if b[8] == 'x' { 't' } else { 'T' };
    }
    let mut s = String::new();
    s.push(t);
    for c in b {
        s.push(c);
    }
    s
}

fn suffix(md: &fs::Metadata, classify: u8) -> &'static str {
    // classify: 1=-F/classify（全後綴含 *）；2=--file-type/file-type（無 *、無裝置）；3=-p/slash（僅目錄）
    if classify == 0 {
        return "";
    }
    let mode = md.permissions().mode();
    match mode & S_IFMT {
        S_IFDIR => "/",
        S_IFLNK if classify <= 2 => "@",
        S_IFIFO if classify <= 2 => "|",
        S_IFSOCK if classify <= 2 => "=",
        S_IFREG if classify == 1 && mode & 0o111 != 0 => "*",
        _ => "",
    }
}

fn humanize(size: u64, si: bool) -> String {
    let units: [&str; 9] = if si {
        ["", "k", "M", "G", "T", "P", "E", "Z", "Y"]
    } else {
        ["", "K", "M", "G", "T", "P", "E", "Z", "Y"]
    };
    let base: f64 = if si { 1000.0 } else { 1024.0 };
    if (size as f64) < base {
        return size.to_string();
    }
    let mut v = size as f64;
    let mut u = 0usize;
    while v >= base && u < 8 {
        v /= base;
        u += 1;
    }
    // 參考慣例（本機 ls 0.10.0，34 sizes sweep 實測）：
    //  - v < 10：2 位有效 + ceil，整數也保留 .0（1024→1.0K、2048→2.0K、1.0498k→1.1k、2.047k→2.1k）。
    //  - 10 ≤ v < base：3 位有效 ceil 整數（10.09K→11K、976.6K→977K）。
    //  - ceil ≥ base → 進一個單位（999999 --si：999.999 → 1000k）。
    // 殘留 byte-exact 差異見 docs/assumptions.md（16385→17K、1048575→1024K、1049600→1.1M 等
    // 近 base² 邊界的 1024/1000 基準切換，無法由單一公式重現）。
    loop {
        if v < 10.0 {
            let scaled = (v * 10.0).ceil();
            return format!("{:.1}{}", scaled / 10.0, units[u]);
        }
        let iv = v.ceil();
        if iv >= base && u < 8 {
            v = iv / base;
            u += 1;
            continue;
        }
        return format!("{}{}", iv, units[u]);
    }
}

fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let ac: Vec<char> = a.chars().collect();
    let bc: Vec<char> = b.chars().collect();
    let (mut i, mut j) = (0, 0);
    while i < ac.len() && j < bc.len() {
        if ac[i].is_ascii_digit() && bc[j].is_ascii_digit() {
            let si = i;
            while i < ac.len() && ac[i].is_ascii_digit() {
                i += 1;
            }
            let sj = j;
            while j < bc.len() && bc[j].is_ascii_digit() {
                j += 1;
            }
            let ta = ac[si..i].iter().collect::<String>();
            let tb = bc[sj..j].iter().collect::<String>();
            let na = ta.trim_start_matches('0');
            let nb = tb.trim_start_matches('0');
            if na.len() != nb.len() {
                return na.len().cmp(&nb.len());
            }
            let ord = na.cmp(nb);
            if ord != std::cmp::Ordering::Equal {
                return ord;
            }
        } else {
            let ord = ac[i].cmp(&bc[j]);
            if ord != std::cmp::Ordering::Equal {
                return ord;
            }
            i += 1;
            j += 1;
        }
    }
    (ac.len() - i).cmp(&(bc.len() - j))
}

fn ext_of(name: &str) -> String {
    match name.rfind('.') {
        Some(p) if p > 0 => name[p..].to_string(),
        _ => String::new(),
    }
}

fn ext_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let ea = ext_of(a);
    let eb = ext_of(b);
    // 副檔名短的在前（無副檔名最短），同長度再字典序
    ea.chars().count()
        .cmp(&eb.chars().count())
        .then_with(|| ea.cmp(&eb))
}

fn sort_entries(v: &mut [Entry], o: &Opt) {
    match o.sort {
        Sort::None => {}
        Sort::Name => v.sort_by(|a, b| a.name.cmp(&b.name)),
        Sort::Size => v.sort_by(|a, b| b.md.len().cmp(&a.md.len()).then_with(|| b.name.cmp(&a.name))),
        // 注：同 size/同時間之 tie 探測定案為名稱降序（uutils 0.10.0 實測）；
        // 同時間戳的 tie 在某些 uutils 内部順序下不保證一致（見 assumptions）。
        Sort::Time => {
            let f = o.time_field;
            v.sort_by(|a, b| {
                time_of(&b.md, f)
                    .cmp(&time_of(&a.md, f))
                    .then_with(|| b.name.cmp(&a.name))
            })
        }
        Sort::Extension => v.sort_by(|a, b| ext_cmp(&a.name, &b.name).then_with(|| a.name.cmp(&b.name))),
        Sort::Version => v.sort_by(|a, b| natural_cmp(&a.name, &b.name).then_with(|| a.name.cmp(&b.name))),
    }
    if o.reverse {
        v.reverse();
    }
    if o.group_dirs_first {
        v.sort_by_key(|e| !is_dirish(e));
    }
}

fn is_dirish(e: &Entry) -> bool {
    match e.md.permissions().mode() & S_IFMT {
        S_IFDIR => true,
        S_IFLNK => fs::metadata(&e.path).map(|md| md.is_dir()).unwrap_or(false),
        _ => false,
    }
}

fn glob_match(pat: &str, name: &str) -> bool {
    fn m(p: &[char], n: &[char]) -> bool {
        if p.is_empty() {
            return n.is_empty();
        }
        match p[0] {
            '*' => (0..=n.len()).any(|k| m(&p[1..], &n[k..])),
            '?' => !n.is_empty() && m(&p[1..], &n[1..]),
            '[' => {
                if n.is_empty() {
                    return false;
                }
                let mut i = 1;
                let neg = p.get(i) == Some(&'!') || p.get(i) == Some(&'^');
                if neg {
                    i += 1;
                }
                let mut hit = false;
                while i < p.len() && p[i] != ']' {
                    if i + 2 < p.len() && p[i + 1] == '-' && p[i + 2] != ']' {
                        if n[0] >= p[i] && n[0] <= p[i + 2] {
                            hit = true;
                        }
                        i += 3;
                    } else {
                        if p[i] == n[0] {
                            hit = true;
                        }
                        i += 1;
                    }
                }
                if i < p.len() && p[i] == ']' {
                    if hit != neg {
                        return m(&p[i + 1..], &n[1..]);
                    }
                }
                false
            }
            c => !n.is_empty() && c == n[0] && m(&p[1..], &n[1..]),
        }
    }
    m(&pat.chars().collect::<Vec<char>>(), &name.chars().collect::<Vec<char>>())
}

fn ignored(name: &str, o: &Opt) -> bool {
    o.ignore.iter().any(|g| glob_match(g, name))
}

// ---- 主流程 ----

fn collect_dir(dir_path: &str, o: &Opt) -> Vec<Entry> {
    let mut out = Vec::new();
    if o.all {
        for n in [".", ".."] {
            let p = if dir_path == "." {
                n.to_string()
            } else {
                format!("{}/{}", dir_path.trim_end_matches('/'), n)
            };
            if let Ok(md) = fs::symlink_metadata(&p) {
                out.push(Entry {
                    name: n.to_string(),
                    path: p,
                    md,
                    link_target: None,
                    link_is_dir: false,
                });
            }
        }
    }
    let rd = match fs::read_dir(dir_path) {
        Ok(rd) => rd,
        Err(_) => return out,
    };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !o.all && !o.almost && name.starts_with('.') {
            continue;
        }
        if ignored(&name, o) {
            continue;
        }
        let path = e.path().to_string_lossy().to_string();
        let md_opt = if o.deref {
            fs::metadata(&path).or_else(|_| fs::symlink_metadata(&path)).ok()
        } else {
            fs::symlink_metadata(&path).ok()
        };
        let md = match md_opt {
            Some(m) => m,
            None => continue,
        };
        let lt = if !o.deref && (md.permissions().mode() & S_IFMT) == S_IFLNK {
            fs::read_link(&path).ok().map(|p| p.to_string_lossy().to_string())
        } else {
            None
        };
        let link_is_dir = lt
            .as_ref()
            .map(|t| resolve_link_dir(&path, t))
            .unwrap_or(false);
        out.push(Entry { name, path, md, link_target: lt, link_is_dir });
    }
    out
}

struct Lister<'a> {
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
    name: &'a str,
    rc: i32,
    bytes: RefCell<u64>,
}

impl Lister<'_> {
    fn w_str(&mut self, s: &str) {
        write!(self.out, "{}", s).ok();
        *self.bytes.borrow_mut() += s.len() as u64;
    }
    fn w_line(&mut self, s: &str) {
        writeln!(self.out, "{}", s).ok();
        *self.bytes.borrow_mut() += s.len() as u64 + 1;
    }
    fn w_bytes(&mut self, n: u64) {
        *self.bytes.borrow_mut() += n;
    }
    fn bytes_now(&self) -> u64 {
        *self.bytes.borrow()
    }
}

fn record_name(o: &Opt, before: u64, len: usize) {
    if o.format == Format::Dired {
        o.dired_offsets.borrow_mut().push((before, before + len as u64));
    }
}

fn shell_quoted(name: &str, style: Quote) -> String {
    let has_special = name
        .chars()
        .any(|c| (c as u32) <= 0x20 || c as u32 == 0x7f || c == '\\');
    let has_squote = name.contains('\'');
    let always = style == Quote::ShellAlways || style == Quote::ShellEscapeAlways;
    let escape_ctl = style == Quote::ShellEscape || style == Quote::ShellEscapeAlways;
    if !has_special && !has_squote && !always {
        return name.to_string();
    }
    if has_squote {
        // 參考實測：含單引號 → C-style 雙引號
        return escape_c(name, true);
    }
    if !escape_ctl {
        let mut out = String::from("'");
        out.push_str(name);
        out.push('\'');
        return out;
    }
    let mut out = String::new();
    let mut normal = String::new();
    let mut ctl = String::new();
    for c in name.chars() {
        if (c as u32) <= 0x20 || c as u32 == 0x7f || c == '\\' {
            if !normal.is_empty() {
                out.push('\'');
                out.push_str(&normal);
                out.push('\'');
                normal.clear();
            }
            match c {
                '\n' => ctl.push_str("\\n"),
                '\r' => ctl.push_str("\\r"),
                '\t' => ctl.push_str("\\t"),
                '\\' => ctl.push_str("\\\\"),
                '\0' => ctl.push_str("\\0"),
                c => ctl.push_str(&format!("\\{:03o}", c as u32)),
            }
        } else {
            if !ctl.is_empty() {
                out.push_str("$'");
                out.push_str(&ctl);
                out.push('\'');
                ctl.clear();
            }
            normal.push(c);
        }
    }
    if !ctl.is_empty() {
        out.push_str("$'");
        out.push_str(&ctl);
        out.push('\'');
    }
    if !normal.is_empty() {
        out.push('\'');
        out.push_str(&normal);
        out.push('\'');
    }
    out
}

fn list_dir(dir_display: &str, dir_path: &str, o: &mut Opt, l: &mut Lister, visited: &mut HashMap<(u64, u64), bool>) {
    let rd = match fs::read_dir(dir_path) {
        Ok(rd) => rd,
        Err(e) => {
            writeln!(l.err, "{}: cannot open directory '{}': {}", l.name, dir_display, short_err(&e)).ok();
            l.rc = 2;
            return;
        }
    };
    drop(rd);
    let mut entries = collect_dir(dir_path, o);
    sort_entries(&mut entries, o);
    print_entries_ctx(&entries, o, l, true);
    if o.recursive {
        let mut subdirs: Vec<&Entry> = entries.iter().filter(|e| is_dirish(e)).collect();
        subdirs.sort_by(|a, b| a.name.cmp(&b.name));
        for e in subdirs {
            let key = (e.md.dev(), e.md.ino());
            if e.md.permissions().mode() & S_IFMT == S_IFDIR && visited.insert(key, true).is_some() {
                continue;
            }
            writeln!(l.out).ok();
            if o.format == Format::Dired {
                l.w_line(&format!("  {}:", e.path));
            } else {
                l.w_line(&format!("{}:", e.path));
            }
            list_dir(&e.path, &e.path, o, l, visited);
        }
    }
}

fn plain_display(e: &Entry, o: &Opt, out: &mut dyn Write) -> String {
    let q = quote_name(&e.name, o);
    let colored = match color_key(e) {
        Some(k) if color_active(o, out) => match o.colors.get(k) {
            Some(code) => {
                if o.quote != Quote::C {
                    wrap_color(code, &q)
                } else {
                    let inner = escape_c(&e.name, false);
                    format!("\"{}\"", wrap_color(code, &inner))
                }
            }
            None => q.clone(),
        },
        _ => q.clone(),
    };
    let linked = if !color_active(o, out) && hyperlink_active(o, out) && !o.zero && o.format != Format::Dired {
        wrap_hyperlink(&hyperlink_uri(e), colored)
    } else {
        colored
    };
    let ctx = if o.context { "? " } else { "" };
    if matches!(o.format, Format::Long | Format::Dired) {
        let lsuf = match e.md.permissions().mode() & S_IFMT {
            S_IFDIR if o.classify >= 1 => "/",
            S_IFIFO if o.classify == 1 => "|",
            S_IFREG if o.classify == 1 && (e.md.permissions().mode() & 0o111) != 0 => "*",
            _ => "",
        };
        format!("{}{}{}", ctx, linked, lsuf)
    } else {
        format!("{}{}{}", ctx, linked, suffix(&e.md, o.classify))
    }
}

fn print_entries(entries: &[Entry], o: &Opt, l: &mut Lister) {
    print_entries_ctx(entries, o, l, false);
}

fn print_entries_ctx(entries: &[Entry], o: &Opt, l: &mut Lister, list_ctx: bool) {
    match o.format {
        Format::OneCol | Format::Commas => {
            if o.format == Format::Commas && !o.zero {
                let mut names: Vec<String> = Vec::with_capacity(entries.len());
                for e in entries {
                    names.push(plain_display(e, o, l.out));
                }
                let width = o.width.unwrap_or(80);
                // 參考斷行規則：（cur 已含前置 ", "）cur_len + next_len > width →
                // 斷行，分隔詞 ", " 保留在行尾。
                let mut cur = String::new();
                for nm in &names {
                    if !cur.is_empty() {
                        cur.push_str(", ");
                    }
                    cur.push_str(nm);
                    // 參考（-m 實測）：名稱區段與分隔詞的數寬規則不完全穩定
                    // （詳 docs/assumptions.md）；此處用 cur+2>=width 近似。
                    if cur.chars().count() + 2 >= width {
                        cur.push_str(", ");
                        l.w_line(cur.trim_end());
                        cur = String::new();
                    }
                }
                if !cur.is_empty() {
                    l.w_line(&cur);
                }
                return;
            }
            if o.zero {
                for e in entries {
                    let mut s = String::new();
                    if o.inode {
                        s.push_str(&format!("{} ", e.md.ino()));
                    }
                    if o.blocks {
                        s.push_str(&format!("{} ", e.md.blocks() / 2));
                    }
                    s.push_str(&quote_name(&e.name, o));
                    write!(l.out, "{}\0", s).ok();
                    l.w_bytes(s.len() as u64 + 1);
                }
                return;
            }
            if o.blocks && list_ctx {
                let total: u64 = entries.iter().map(|e| e.md.blocks()).sum::<u64>() / 2;
                l.w_line(&format!("total {}", total));
            }
            let iw = if o.inode { entries.iter().map(|e| e.md.ino().to_string().len()).max().unwrap_or(0) } else { 0 };
            let bw = if o.blocks { entries.iter().map(|e| (e.md.blocks() / 2).to_string().len()).max().unwrap_or(0) } else { 0 };
            for e in entries {
                let mut prefix = String::new();
                if o.inode {
                    prefix.push_str(&format!("{:>width$} ", e.md.ino(), width = iw));
                }
                if o.blocks {
                    prefix.push_str(&format!("{:>width$} ", e.md.blocks() / 2, width = bw));
                }
                l.w_str(&prefix);
                let disp = plain_display(e, o, l.out);
                record_name(o, l.bytes_now(), disp.len());
                l.w_str(&disp);
                l.w_line("");
            }
        }
        Format::Columns | Format::Rows => {
            if o.blocks && list_ctx {
                let total: u64 = entries.iter().map(|e| e.md.blocks()).sum::<u64>() / 2;
                l.w_line(&format!("total {}", total));
            }
            let iw = if o.inode { entries.iter().map(|e| e.md.ino().to_string().len()).max().unwrap_or(0) } else { 0 };
            let bw = if o.blocks { entries.iter().map(|e| (e.md.blocks() / 2).to_string().len()).max().unwrap_or(0) } else { 0 };
            let strings: Vec<String> = {
                let mut v: Vec<String> = Vec::with_capacity(entries.len());
                for e in entries {
                    let mut s = String::new();
                    if o.inode {
                        s.push_str(&format!("{:>width$} ", e.md.ino(), width = iw));
                    }
                    if o.blocks {
                        s.push_str(&format!("{:>width$} ", e.md.blocks() / 2, width = bw));
                    }
                    s.push_str(&plain_display(e, o, l.out));
                    v.push(s);
                }
                v
            };
            if o.blocks && list_ctx {
                let total: u64 = entries.iter().map(|e| e.md.blocks()).sum::<u64>() / 2;
                l.w_line(&format!("total {}", total));
            }
            print_columns(&strings, entries, o.format, o, l);
        }
        Format::Long | Format::Dired => print_long(entries, o, l, list_ctx),
    }
}

fn print_columns(strings: &[String], entries: &[Entry], format: Format, o: &Opt, l: &mut Lister) {
    let n = strings.len();
    if n == 0 {
        return;
    }
    let width = o.width.unwrap_or(80);
    let widths: Vec<usize> = strings.iter().map(|s| s.chars().count()).collect();
    // 參考演算法（uutils 0.10.0 實測）：找最小列數 r0 使 k=ceil(n/r0) 欄可容納
    // （欄寬=各欄 max＋2、末欄不補）；欄成員為.contiguous 分群（column-major）。
    let mut chosen: Option<(usize, Vec<usize>)> = None;
    for r0 in 1..=n {
        let k = (n + r0 - 1) / r0;
        if format == Format::Columns && (n + k - 1) / k != r0 {
            // r0 必須等於 ceil(n/k)，否則欄群不对齊
        }
        let mut colw: Vec<usize> = Vec::new();
        let mut sum = 0usize;
        for c in 0..k {
            let mx = if format == Format::Rows {
                let mut m = 0usize;
                let mut idx = c;
                while idx < n {
                    m = std::cmp::max(m, widths[idx]);
                    idx += k;
                }
                m
            } else {
                let start = c * r0;
                let end = std::cmp::min(start + r0, n);
                *widths[start..end].iter().max().unwrap_or(&0)
            };
            colw.push(mx);
            sum += if c + 1 < k { mx + 2 } else { mx };
        }
        if sum <= width {
            chosen = Some((r0, colw));
            break;
        }
    }
    let (rows, colw) = chosen.unwrap_or((n, vec![*widths.iter().max().unwrap_or(&0)]));
    let k = (n + rows - 1) / rows;
    let tab = if o.tabsize == 0 { usize::MAX } else { o.tabsize };
    let suffixes: Vec<&'static str> = entries.iter().map(|e| suffix(&e.md, o.classify)).collect();
    for r in 0..rows {
        let mut pos = 0usize;
        for c in 0..k {
            let idx = if format == Format::Rows { r * k + c } else { c * rows + r };
            if idx >= n {
                break;
            }
            l.w_str(&strings[idx]);
            pos += strings[idx].chars().count();
            let _ = &suffixes;
            // 參考：行內最後一個實際項目之後不補空白（幻影欄不觸發補白）。
            let next_idx = if format == Format::Rows { idx + 1 } else { idx + rows };
            if c + 1 < k && next_idx < n {
                let target = pos + (colw[c] + 2 - strings[idx].chars().count());
                while pos < target {
                    let stop = (pos / tab + 1) * tab;
                    if pos % tab != 0 && stop <= target {
                        l.w_str("\t");
                        pos = stop;
                    } else {
                        l.w_str(" ");
                        pos += 1;
                    }
                }
            }
        }
        l.w_line("");
    }
}

fn size_text(e: &Entry, o: &Opt) -> String {
    let mode = e.md.permissions().mode();
    match mode & S_IFMT {
        S_IFCHR | S_IFBLK => {
            let r = e.md.rdev();
            format!("{}, {}", (r >> 8) & 0xfff, r & 0xff)
        }
        _ => {
            if o.human != 0 {
                // 參考 quirk：-h/--si 僅 humanize total（blocks*512）；
                // 個別檔案的大小欄仍是 st_size 原值。
                humanize(e.md.len(), o.human == 2)
            } else {
                e.md.len().to_string()
            }
        }
    }
}

fn print_long(entries: &[Entry], o: &Opt, l: &mut Lister, list_ctx: bool) {
    let with_total = o.blocks || (list_ctx && matches!(o.format, Format::Long | Format::Dired));
    if with_total {
        let blocks: u64 = entries.iter().map(|e| e.md.blocks()).sum();
        let total = blocks / 2;
        let shown = if o.human != 0 {
            humanize(blocks * 512, o.human == 2)
        } else {
            total.to_string()
        };
        if o.format == Format::Dired {
            l.w_line(&format!("  total {}", shown));
        } else {
            l.w_line(&format!("total {}", shown));
        }
    }
    let mut ow = 0usize;
    let mut gw = 0usize;
    let mut nw = 0usize;
    let mut sw = 0usize;
    let mut bw = 0usize;
    let mut iw = 0usize;
    let style = if o.full_time { TimeStyle::FullIso } else { o.time_style.clone() };
    let mut times: Vec<String> = Vec::new();
    for e in entries {
        ow = std::cmp::max(ow, crate::chown_common::name_of(e.md.uid(), false).chars().count());
        gw = std::cmp::max(gw, crate::chown_common::name_of(e.md.gid(), true).chars().count());
        nw = std::cmp::max(nw, e.md.nlink().to_string().len());
        // 參考 quirk：tmpfs 且列表含 '..' 時，'..' 的 st_size 不參與大小欄寬 sw
        // （其欄寬僅影響 '..' 行自身）。
        if !(tmpfs_dev(&e.md) && e.name == "..") {
            sw = std::cmp::max(sw, size_text(e, o).chars().count());
        }
        bw = std::cmp::max(bw, (e.md.blocks() / 2).to_string().len());
        iw = std::cmp::max(iw, e.md.ino().to_string().len());
        let (sec, nsec) = time_of(&e.md, o.time_field);
        times.push(fmt_time(sec, nsec, &style, false));
    }
    for (i, e) in entries.iter().enumerate() {
        let mut line = String::new();
        if o.format == Format::Dired {
            line.push_str("  ");
        }
        if o.inode {
            line.push_str(&format!("{:>width$} ", e.md.ino(), width = iw));
        }
        if o.blocks {
            line.push_str(&format!("{} ", e.md.blocks() / 2));
        }
        line.push_str(&mode_string(&e.md));
        line.push_str(&format!(" {:>width$}", e.md.nlink().to_string(), width = nw));
        let tmpfs = tmpfs_dev(&e.md);
        let size_str = size_text(e, o);
        // 參考 quirk（tmpfs/ext4 實測、見 docs/assumptions.md）：
        //  - 大小欄 sw = max（不含 tmpfs 的 '..'）；每行 sw_line = max(sw, 自身寬)。
        //  - tmpfs：'..' 行 owner pad 到 gw+1（含前置空格共 gw+2 寬）、group 恒單空格前置。
        //  - ext4：恒單空格。
        let dotdot_row = tmpfs && e.name == "..";
        let pad_owner = dotdot_row;
        let pad_group = dotdot_row;
        let sw_line = std::cmp::max(sw, size_str.chars().count());
        if !o.no_owner {
            let owner = if o.numeric_ids {
                e.md.uid().to_string()
            } else {
                crate::chown_common::name_of(e.md.uid(), false)
            };
            if pad_owner {
                line.push_str(&format!(" {:<width$}", owner, width = gw + 1));
            } else {
                line.push_str(&format!(" {}", owner));
            }
        }
        if !o.no_group {
            let group = if o.numeric_ids {
                e.md.gid().to_string()
            } else {
                crate::chown_common::name_of(e.md.gid(), true)
            };
            if pad_group {
                line.push_str(&format!(" {}", group));
            } else {
                line.push_str(&format!(" {}", group));
            }
        }
        line.push_str(&format!(" {:>width$}", size_str, width = sw_line));
        line.push(' ');
        line.push_str(&times[i]);
        if o.context {
            line.push_str(" ?");
        }
        line.push(' ');
        l.w_str(&line);
        let link = match &e.link_target {
            Some(t) => {
                let tsuf = if o.classify == 1 && e.link_is_dir { "/" } else { "" };
                format!(" -> {}{}", t, tsuf)
            }
            None => String::new(),
        };
        if o.format == Format::Dired {
            let mut disp = quote_name(&e.name, o);
            if !color_active(o, l.out) && hyperlink_active(o, l.out) && !o.zero {
                disp = wrap_hyperlink(&hyperlink_uri(e), disp);
            }
            record_name(o, l.bytes_now(), disp.len());
            l.w_str(&disp);
        } else {
            let disp = plain_display(e, o, l.out);
            record_name(o, l.bytes_now(), disp.len());
            l.w_str(&disp);
        }
        if !link.is_empty() {
            l.w_str(&link);
        }
        l.w_line("");
    }
}

fn run_mode(mode: u8, args: &[String], _stdin: crate::In, stdout: crate::Out, stderr: crate::Out) -> i32 {
    let name = match mode {
        1 => "dir",
        2 => "vdir",
        _ => "ls",
    };
    let (mut o, operands) = match parse(name, mode, args, stdout, stderr) {
        Ok(v) => v,
        Err(rc) => return rc,
    };
    let mut l = Lister { out: stdout, err: stderr, name, rc: 0, bytes: RefCell::new(0) };
    let operands = if operands.is_empty() { vec![".".to_string()] } else { operands };

    let _single_dir = operands.len() == 1 && {
        match fs::metadata(&operands[0]) {
            Ok(m) => m.is_dir(),
            Err(_) => false,
        }
    };
    let entry_mode = o.dir_mode;
    let mut resolved: Vec<(String, Option<fs::Metadata>)> = Vec::new();
    for p in &operands {
        let md = if o.dir_mode {
            fs::symlink_metadata(p)
        } else if o.deref || (o.deref_cmdline && operands.len() > 1) {
            fs::metadata(p)
        } else {
            fs::symlink_metadata(p)
        };
        resolved.push((p.clone(), md.ok()));
    }
    if let [p] = operands.as_slice() {
        if resolved[0].1.is_none() && fs::symlink_metadata(p).is_err() {
            writeln!(l.err, "{}: cannot access '{}': {}", name, p, short_err(&fs::symlink_metadata(p).unwrap_err())).ok();
            l.rc = 2;
            dired_trailer(&o, &mut l);
            return l.rc;
        }
    }
    let mut files: Vec<(String, fs::Metadata)> = Vec::new();
    let mut dirs: Vec<String> = Vec::new();
    for (p, md) in &resolved {
        match md {
            Some(m) if m.is_dir() => dirs.push(p.clone()),
            Some(_) => files.push((p.clone(), md.clone().unwrap())),
            None => {
                writeln!(l.err, "{}: cannot access '{}': {}", l.name, p, "No such file or directory").ok();
                l.rc = 2;
            }
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    dirs.sort();
    if entry_mode {
        let mut all: Vec<(String, fs::Metadata)> = files.clone();
        for d in &dirs {
            if let Ok(md) = fs::symlink_metadata(d) {
                all.push((d.clone(), md));
            }
        }
        all.sort_by(|a, b| a.0.cmp(&b.0));
        let entries: Vec<Entry> = all
            .iter()
            .filter_map(|(p, md)| match build_entry(p, md, &o) {
                Ok(e) => Some(e),
                Err(msg) => {
                    writeln!(l.err, "{}: cannot access '{}': {}", l.name, p, msg).ok();
                    l.rc = 2;
                    None
                }
            })
            .collect();
        if !entries.is_empty() {
            print_entries(&entries, &o, &mut l);
        }
        dired_trailer(&o, &mut l);
        return l.rc;
    }
    let multi = operands.len() > 1;
    let header = multi || o.recursive;
    let mut printed_any = false;
    if !files.is_empty() {
        let entries: Vec<Entry> = files
            .iter()
            .filter_map(|(p, md)| match build_entry(p, md, &o) {
                Ok(e) => Some(e),
                Err(msg) => {
                    writeln!(l.err, "{}: cannot access '{}': {}", l.name, p, msg).ok();
                    l.rc = 2;
                    None
                }
            })
            .collect();
        if !entries.is_empty() {
            print_entries(&entries, &o, &mut l);
            printed_any = true;
        }
    }
    for d in &dirs {
        if printed_any {
            l.w_line("");
        }
        printed_any = true;
        if header {
            if o.format == Format::Dired {
                l.w_line(&format!("  {}:", d));
            } else {
                l.w_line(&format!("{}:", d));
            }
        }
        list_dir(d, d, &mut o, &mut l, &mut HashMap::new());
    }
    dired_trailer(&o, &mut l);
    l.rc
}

fn tmpfs_dev(md: &fs::Metadata) -> bool {
    let dev = md.dev();
    let mounts = match fs::read_to_string("/proc/mounts") {
        Ok(v) => v,
        Err(_) => return false,
    };
    for line in mounts.lines() {
        let mut it = line.split_whitespace();
        let (_src, mnt, fstype) = match (it.next(), it.next(), it.next()) {
            (Some(a), Some(b), Some(c)) => (a, b, c),
            _ => continue,
        };
        if fstype != "tmpfs" {
            continue;
        }
        if let Ok(m) = fs::metadata(mnt) {
            if m.dev() == dev {
                return true;
            }
        }
    }
    false
}

fn resolve_link_dir(entry_path: &str, target: &str) -> bool {
    if target.starts_with('/') {
        fs::metadata(target).map(|m| m.is_dir()).unwrap_or(false)
    } else {
        let dir = entry_path.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_else(|| ".".to_string());
        let joined = if dir == "." { target.to_string() } else { format!("{}/{}", dir, target) };
        fs::metadata(joined).map(|m| m.is_dir()).unwrap_or(false)
    }
}

fn build_entry(p: &str, md: &fs::Metadata, o: &Opt) -> Result<Entry, String> {
    let is_link = (md.permissions().mode() & S_IFMT) == S_IFLNK;
    let mut md2 = md.clone();
    if !o.dir_mode && (o.deref || (o.deref_cmdline && is_link)) && is_link {
        md2 = match fs::metadata(p) {
            Ok(m) => m,
            Err(e) => return Err(short_err(&e)),
        };
    }
    let lt = if is_link { fs::read_link(p).ok().map(|x| x.to_string_lossy().to_string()) } else { None };
    let link_is_dir = lt
        .as_ref()
        .map(|t| resolve_link_dir(p, t))
        .unwrap_or(false);
    Ok(Entry { name: display_operand_name(p, md), path: p.to_string(), md: md2, link_target: lt, link_is_dir })
}

fn dired_trailer(o: &Opt, l: &mut Lister) {
    if o.format != Format::Dired {
        return;
    }
    let offs: Vec<String> = o.dired_offsets.borrow().iter().map(|(a, b)| format!("{} {}", a, b)).collect();
    if !offs.is_empty() {
        writeln!(l.out, "//DIRED// {}", offs.join(" ")).ok();
    }
    writeln!(l.out, "//DIRED-OPTIONS// --quoting-style={}", style_name(o.quote)).ok();
}

fn display_operand_name(p: &str, _md: &fs::Metadata) -> String {
    // file operand 列出時名稱＝原樣（ls -l f1 → f1；ls -l /a/b → /a/b）
    p.to_string()
}
