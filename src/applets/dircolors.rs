//! dircolors(1) — Output shell code to set LS_COLORS.
//!
//! 行為依本機參考實作（uutils coreutils 0.10.0）逐 case diff（docs/probes/cluster5.md）：
//! - 預設資料庫內嵌於本檔（與 docs/probes/dircolors-db.txt 逐 byte 相同；
//!   `dircolors /tmp/db_default.txt` 輸出與 `dircolors` 相同 → 內嵌 db 即可重現預設）
//! - 行解析：跳過 `#` 與空行；第一個空白/Tab 切 key/rest；
//!   rest 空 → "dircolors: <file>:<lineno>: invalid line;  missing second token"（兩個空格）
//!   key 以 `*`/`.` 開頭 → 副檔名條目；recognised keyword → 短碼；
//!   否則 "dircolors: unrecognized keyword <KEY>"（KEY 含 `=` 時整串出現）
//! - TERM/COLORTERM 是 glob filter：其**之後**的條目依 $TERM/$COLORTERM 匹配與否決定納入；
//!   filter 行之前的條目恒納入
//! - 輸出 `LS_COLORS='…:';` + `export LS_COLORS`；-c → `setenv LS_COLORS '…'`
//! - 兩 operand → "dircolors: extra operand 'X'" + Try 行；不存在檔案帶 "(os error 2)" 後綴

use crate::pathname::usage_hint;
use crate::{In, Out};

const DIRCOLORS_USAGE: &str = "Usage: dircolors [OPTION]... [FILE]";

const DIRCOLORS_HELP: &str = r#"Output commands to set the LS_COLORS environment variable.

Usage: dircolors [OPTION]... [FILE]

Options:
  -b, --sh               output Bourne shell code to set LS_COLORS [alias: --bourne-shell]
  -c, --csh              output C shell code to set LS_COLORS [alias: --c-shell]
  -p, --print-database   output the contents of the internal colors database
      --print-ls-colors  output fully escaped colors for display
  -h, --help             Print help
  -V, --version          Print version

If FILE is specified, read it to determine which colors to use for which
file types and extensions. Otherwise, a precompiled database is used.
For details on the format of these files, run 'dircolors --print-database'
"#;

/// clap 風格的長選項候選（用於前綴推斷與 unknown-option tip）。
const DIRCOLORS_LONGS: &[&str] = &[
    "bourne-shell",
    "c-shell",
    "csh",
    "help",
    "print-database",
    "print-ls-colors",
    "sh",
    "version",
];

const DEFAULT_DB: &str = concat!(
    "# Configuration file for dircolors, a utility to help you set the\n",
    "# LS_COLORS environment variable used by GNU ls with the --color option.\n",
    "# The keywords COLOR, OPTIONS, and EIGHTBIT (honored by the\n",
    "# slackware version of dircolors) are recognized but ignored.\n",
    "# Global config options can be specified before TERM or COLORTERM entries\n",
    "# Below are TERM or COLORTERM entries, which can be glob patterns, which\n",
    "# restrict following config to systems with matching environment variables.\n",
    "COLORTERM ?*\n",
    "TERM Eterm\n",
    "TERM alacritty*\n",
    "TERM ansi\n",
    "TERM *color*\n",
    "TERM con[0-9]*x[0-9]*\n",
    "TERM cons25\n",
    "TERM console\n",
    "TERM cygwin\n",
    "TERM *direct*\n",
    "TERM dtterm\n",
    "TERM foot\n",
    "TERM gnome\n",
    "TERM hurd\n",
    "TERM jfbterm\n",
    "TERM konsole\n",
    "TERM kterm\n",
    "TERM linux\n",
    "TERM linux-c\n",
    "TERM mlterm\n",
    "TERM putty\n",
    "TERM rxvt*\n",
    "TERM screen*\n",
    "TERM st\n",
    "TERM terminator\n",
    "TERM tmux*\n",
    "TERM vt100\n",
    "TERM wezterm*\n",
    "TERM xterm*\n",
    "# Below are the color init strings for the basic file types.\n",
    "# One can use codes for 256 or more colors supported by modern terminals.\n",
    "# The default color codes use the capabilities of an 8 color terminal\n",
    "# with some additional attributes as per the following codes:\n",
    "# Attribute codes:\n",
    "# 00=none 01=bold 04=underscore 05=blink 07=reverse 08=concealed\n",
    "# Text color codes:\n",
    "# 30=black 31=red 32=green 33=yellow 34=blue 35=magenta 36=cyan 37=white\n",
    "# Background color codes:\n",
    "# 40=black 41=red 42=green 43=yellow 44=blue 45=magenta 46=cyan 47=white\n",
    "#NORMAL 00 # no color code at all\n",
    "#FILE 00 # regular file: use no color at all\n",
    "RESET 0\n",
    "DIR 01;34\n",
    "LINK 01;36\n",
    "MULTIHARDLINK 00\n",
    "FIFO 40;33\n",
    "SOCK 01;35\n",
    "DOOR 01;35\n",
    "BLK 40;33;01\n",
    "CHR 40;33;01\n",
    "ORPHAN 40;31;01\n",
    "MISSING 00\n",
    "SETUID 37;41\n",
    "SETGID 30;43\n",
    "CAPABILITY 00\n",
    "STICKY_OTHER_WRITABLE 30;42\n",
    "OTHER_WRITABLE 34;42\n",
    "STICKY 37;44\n",
    "EXEC 01;32\n",
    "# List any file extensions like '.gz' or '.tar' that you would like ls\n",
    "# to color below. Put the extension, a space, and the color init string.\n",
    ".tar 01;31\n",
    ".tgz 01;31\n",
    ".arc 01;31\n",
    ".arj 01;31\n",
    ".taz 01;31\n",
    ".lha 01;31\n",
    ".lz4 01;31\n",
    ".lzh 01;31\n",
    ".lzma 01;31\n",
    ".tlz 01;31\n",
    ".txz 01;31\n",
    ".tzo 01;31\n",
    ".t7z 01;31\n",
    ".zip 01;31\n",
    ".z 01;31\n",
    ".dz 01;31\n",
    ".gz 01;31\n",
    ".lrz 01;31\n",
    ".lz 01;31\n",
    ".lzo 01;31\n",
    ".xz 01;31\n",
    ".zst 01;31\n",
    ".tzst 01;31\n",
    ".bz2 01;31\n",
    ".bz 01;31\n",
    ".tbz 01;31\n",
    ".tbz2 01;31\n",
    ".tz 01;31\n",
    ".deb 01;31\n",
    ".rpm 01;31\n",
    ".jar 01;31\n",
    ".war 01;31\n",
    ".ear 01;31\n",
    ".sar 01;31\n",
    ".rar 01;31\n",
    ".alz 01;31\n",
    ".ace 01;31\n",
    ".zoo 01;31\n",
    ".cpio 01;31\n",
    ".7z 01;31\n",
    ".rz 01;31\n",
    ".cab 01;31\n",
    ".wim 01;31\n",
    ".swm 01;31\n",
    ".dwm 01;31\n",
    ".esd 01;31\n",
    ".avif 01;35\n",
    ".jpg 01;35\n",
    ".jpeg 01;35\n",
    ".mjpg 01;35\n",
    ".mjpeg 01;35\n",
    ".gif 01;35\n",
    ".bmp 01;35\n",
    ".pbm 01;35\n",
    ".pgm 01;35\n",
    ".ppm 01;35\n",
    ".tga 01;35\n",
    ".xbm 01;35\n",
    ".xpm 01;35\n",
    ".tif 01;35\n",
    ".tiff 01;35\n",
    ".png 01;35\n",
    ".svg 01;35\n",
    ".svgz 01;35\n",
    ".mng 01;35\n",
    ".pcx 01;35\n",
    ".mov 01;35\n",
    ".mpg 01;35\n",
    ".mpeg 01;35\n",
    ".m2v 01;35\n",
    ".mkv 01;35\n",
    ".webm 01;35\n",
    ".webp 01;35\n",
    ".ogm 01;35\n",
    ".mp4 01;35\n",
    ".m4v 01;35\n",
    ".mp4v 01;35\n",
    ".vob 01;35\n",
    ".qt 01;35\n",
    ".nuv 01;35\n",
    ".wmv 01;35\n",
    ".asf 01;35\n",
    ".rm 01;35\n",
    ".rmvb 01;35\n",
    ".flc 01;35\n",
    ".avi 01;35\n",
    ".fli 01;35\n",
    ".flv 01;35\n",
    ".gl 01;35\n",
    ".dl 01;35\n",
    ".xcf 01;35\n",
    ".xwd 01;35\n",
    ".yuv 01;35\n",
    ".cgm 01;35\n",
    ".emf 01;35\n",
    ".ogv 01;35\n",
    ".ogx 01;35\n",
    ".aac 00;36\n",
    ".au 00;36\n",
    ".flac 00;36\n",
    ".m4a 00;36\n",
    ".mid 00;36\n",
    ".midi 00;36\n",
    ".mka 00;36\n",
    ".mp3 00;36\n",
    ".mpc 00;36\n",
    ".ogg 00;36\n",
    ".ra 00;36\n",
    ".wav 00;36\n",
    ".oga 00;36\n",
    ".opus 00;36\n",
    ".spx 00;36\n",
    ".xspf 00;36\n",
    "*~ 00;90\n",
    "*# 00;90\n",
    ".bak 00;90\n",
    ".old 00;90\n",
    ".orig 00;90\n",
    ".part 00;90\n",
    ".rej 00;90\n",
    ".swp 00;90\n",
    ".tmp 00;90\n",
    ".dpkg-dist 00;90\n",
    ".dpkg-old 00;90\n",
    ".ucf-dist 00;90\n",
    ".ucf-new 00;90\n",
    ".ucf-old 00;90\n",
    ".rpmnew 00;90\n",
    ".rpmorig 00;90\n",
    ".rpmsave 00;90\n",
    "# Subsequent TERM or COLORTERM entries, can be used to add / override\n",
    "# config specific to those matching environment variables.\n",
);

/// keyword → LS_COLORS 短碼（大小寫不拘）。
fn keyword_code(key: &str) -> Option<&'static str> {
    let k = key.to_ascii_lowercase();
    Some(match k.as_str() {
        "reset" => "rs",
        "dir" => "di",
        "link" | "lnk" => "ln",
        "multihardlink" => "mh",
        "fifo" => "pi",
        "sock" => "so",
        "door" => "do",
        "blk" => "bd",
        "chr" => "cd",
        "orphan" => "or",
        "missing" => "mi",
        "setuid" | "suid" => "su",
        "setgid" | "sgid" => "sg",
        "capability" => "ca",
        "sticky_other_writable" => "tw",
        "other_writable" | "owr" => "ow",
        "sticky" => "st",
        "exec" | "exe" => "ex",
        // GNU dircolors 的別名（uutils man 未列 keyword 清單 → 本機自訂並記錄於 assumptions）
        "normal" | "file" => "fi",
        "pipe" => "pi",
        "multi" => "mh",
        "block.special" => "bd",
        "char.special" => "cd",
        "left" => "lc",
        "right" => "rc",
        _ => return None,
    })
}

/// 識別但忽略（不產生條目）。
fn ignored_keyword(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "color" | "options" | "eightbit"
    )
}

/// glob：`*`、`?`（uutils 的 TERM/COLORTERM pattern）。
fn glob_match(pat: &str, s: &str) -> bool {
    let p: Vec<char> = pat.chars().collect();
    let t: Vec<char> = s.chars().collect();
    fn m(p: &[char], t: &[char]) -> bool {
        if p.is_empty() {
            return t.is_empty();
        }
        match p[0] {
            '*' => {
                for i in 0..=t.len() {
                    if m(&p[1..], &t[i..]) {
                        return true;
                    }
                }
                false
            }
            '?' => !t.is_empty() && m(&p[1..], &t[1..]),
            c => !t.is_empty() && t[0] == c && m(&p[1..], &t[1..]),
        }
    }
    m(&p, &t)
}

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut csh = false;
    let mut print_db = false;
    let mut print_ls = false;
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
            let name = crate::pathname::infer_long_opt(&name, DIRCOLORS_LONGS)
            .map(|full| full.to_string())
            .unwrap_or(name);
            match name.as_str() {
                "sh" | "bourne-shell" => csh = false,
                "csh" | "c-shell" => csh = true,
                "print-database" => print_db = true,
                "print-ls-colors" => print_ls = true,
                "help" => {
                    write!(stdout, "{}", DIRCOLORS_HELP).ok();
                    return 0;
                }
                "version" => {
                    writeln!(stdout, "dircolors (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                _ => return unexpected(&a, stderr),
            }
            continue;
        }
        for c in a[1..].chars() {
            match c {
                'b' => csh = false,
                'c' => csh = true,
                'p' => print_db = true,
                'h' => {
                    write!(stdout, "{}", DIRCOLORS_HELP).ok();
                    return 0;
                }
                'V' => {
                    writeln!(stdout, "dircolors (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                c => return unexpected(&format!("-{}", c), stderr),
            }
        }
    }

    if print_db {
        let db = if operands.is_empty() {
            DEFAULT_DB.to_string()
        } else {
            match std::fs::read_to_string(&operands[0]) {
                Ok(s) => s,
                Err(e) => {
                    writeln!(stderr, "dircolors: {}: {}", operands[0], e).ok();
                    return 1;
                }
            }
        };
        write!(stdout, "{}", db).ok();
        return 0;
    }

    if operands.len() > 1 {
        writeln!(stderr, "dircolors: extra operand '{}'", operands[1]).ok();
        writeln!(stderr, "{}", usage_hint("dircolors")).ok();
        return 1;
    }

    let db = if operands.is_empty() {
        DEFAULT_DB.to_string()
    } else {
        match std::fs::read_to_string(&operands[0]) {
            Ok(s) => s,
            Err(e) => {
                // 此處參考保留 "(os error N)" 後綴
                writeln!(stderr, "dircolors: {}: {}", operands[0], e).ok();
                return 1;
            }
        }
    };

    let name = operands.first().cloned().unwrap_or_else(|| "internal database".to_string());
    let mut entries: Vec<(String, String)> = Vec::new();
    let mut active = true;
    for (idx, line) in db.lines().enumerate() {
        let lineno = idx + 1;
        let trimmed = line.trim_end_matches('\r');
        let t = trimmed.trim_start();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        // 以第一個空白/Tab 切 key / rest
        let pos = match t.find(|c: char| c == ' ' || c == '\t') {
            Some(p) => p,
            None => {
                writeln!(
                    stderr,
                    "dircolors: {}: {}: invalid line;  missing second token",
                    name, lineno
                )
                .ok();
                return 1;
            }
        };
        let key = &t[..pos];
        let rest = t[pos + 1..].trim();
        if rest.is_empty() {
            writeln!(
                stderr,
                "dircolors: {}: {}: invalid line;  missing second token",
                name, lineno
            )
            .ok();
            return 1;
        }
        if key == "TERM" || key == "COLORTERM" {
            let env = if key == "TERM" {
                std::env::var("TERM").unwrap_or_default()
            } else {
                std::env::var("COLORTERM").unwrap_or_default()
            };
            active = glob_match(rest, &env);
            continue;
        }
        if ignored_keyword(key) {
            continue;
        }
        if !active {
            continue;
        }
        let code = if key.starts_with('*') || key.starts_with('.') {
            if key.starts_with('.') {
                format!("*{}", key)
            } else {
                key.to_string()
            }
        } else if let Some(c) = keyword_code(key) {
            c.to_string()
        } else {
            writeln!(stderr, "dircolors: unrecognized keyword {}", key).ok();
            return 1;
        };
        // value = rest 原樣（可含空白）；extension key 含 '=' 時不重複 key
        // （例 `*.tar=01;31 x` → `*.tar=01;31=x:`）
        let value = rest.to_string();
        entries.push((code, value));
    }

    if print_ls {
        for (code, value) in &entries {
            write!(stdout, "\x1b[{}m{}\t{}\x1b[0m\n", value, code, value).ok();
        }
        return 0;
    }

    let mut joined = String::new();
    for (code, value) in &entries {
        joined.push_str(&code);
        joined.push('=');
        joined.push_str(value);
        joined.push(':');
    }
    if csh {
        writeln!(stdout, "setenv LS_COLORS '{}'", joined).ok();
    } else {
        writeln!(stdout, "LS_COLORS='{}';", joined).ok();
        writeln!(stdout, "export LS_COLORS").ok();
    }
    0
}

fn unexpected(arg: &str, stderr: Out) -> i32 {
    writeln!(stderr, "error: unexpected argument '{}' found", arg).ok();
    writeln!(stderr).ok();
    // clap：未知長選項若為已知長選項的前綴 → 給 similar tip；否則給 value tip
    let tip = if let Some(short) = arg.strip_prefix("--") {
        match DIRCOLORS_LONGS.iter().find(|l| l.starts_with(short)) {
            Some(l) => format!("tip: a similar argument exists: '--{}'", l),
            None => format!("  tip: to pass '{}' as a value, use '-- {}'", arg, arg),
        }
    } else {
        format!("  tip: to pass '{}' as a value, use '-- {}'", arg, arg)
    };
    writeln!(stderr, "{}", tip).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "{}", DIRCOLORS_USAGE).ok();
    writeln!(stderr, "For more information, try '--help'.").ok();
    1
}
