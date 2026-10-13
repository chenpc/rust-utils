//! stty — 印出或修改終端機特性。
//!
//! 規格來源：`docs/man/stty.1.gz`（OPTIONS：-a/--all、-g/--save、-F/--file DEVICE、-h、-V、[settings]）。
//! man 未定義 settings 詞表與輸出格式 → 我方決定（見 docs/assumptions.md「stty」節）：
//! - 支援的 settings：echo/-echo 等 termios 旗標名（icanon、isig、iexten、ixon… 見 FLAG 表）、
//!   raw、cooked、sane、size、rows N、cols N、speed N、ispeed N、ospeed N、cchars 復原 token（VINTR=0x03）。
//! - 無任何選項與 settings：只印 `speed <ospeed> baud;`（精簡摘要）。
//! - -a 用人類可讀三行：speed 行、旗標行（開=名、關=-名）、cchars 行。
//! - -g 輸出單一行的 stty 可讀形式（ispeed/ospeed + 全部旗標 + cchars NAME=0xNN），
//!   且本實作的 settings 解析器保證可讀回自己（round-trip 不變量）。

use std::fs::File;
use std::os::unix::io::AsRawFd;

use crate::pathname;
use crate::sys;
use crate::{In, Out};

const STTY_HELP: &str = r#"Print or change terminal characteristics.

Usage: stty [OPTION]... [SETTING]...

Options:
  -a, --all          print all current settings in human-readable form
  -g, --save         print all current settings in a stty-readable form
  -F, --file DEVICE  open and use the specified DEVICE instead of stdin
  -h, --help         Print help
  -V, --version      Print version

Arguments:
  [settings]  settings to change
"#;

const STTY_LONGS: &[&str] = &["all", "file", "help", "save", "version"];

fn unexpected(arg: &str, stderr: Out) -> i32 {
    write!(stderr, "error: unexpected argument '{}' found\n\n", arg).ok();
    write!(
        stderr,
        "Usage: stty [OPTION]... [SETTING]...\n\nFor more information, try '--help'.\n"
    )
    .ok();
    2
}

fn io_err(label: &str, e: &std::io::Error, stderr: Out) -> i32 {
    writeln!(stderr, "stty: {}: {}", label, pathname::io_reason_nosuffix(e)).ok();
    1
}

// (name, class, on-bits)；class: 0=iflag 1=oflag 2=cflag 3=lflag。CSIZE 群用 mask 語意處理。
const FLAGS: &[(&str, u8, u32)] = &[
    ("ignbrk", 0, sys::IGNBRK),
    ("brkint", 0, sys::BRKINT),
    ("ignpar", 0, sys::IGNPAR),
    ("parmrk", 0, sys::PARMRK),
    ("inpck", 0, sys::INPCK),
    ("istrip", 0, sys::ISTRIP),
    ("inlcr", 0, sys::INLCR),
    ("igncr", 0, sys::IGNCR),
    ("icrnl", 0, sys::ICRNL),
    ("ixon", 0, sys::IXON),
    ("ixoff", 0, sys::IXOFF),
    ("ixany", 0, sys::IXANY),
    ("opost", 1, sys::OPOST),
    ("onlcr", 1, sys::ONLCR),
    ("ocrnl", 1, sys::OCRNL),
    ("onocr", 1, sys::ONOCR),
    ("onlret", 1, sys::ONLRET),
    ("cs5", 2, 999), // sentinel：CSIZE 群
    ("cs6", 2, 998),
    ("cs7", 2, 997),
    ("cs8", 2, 996),
    ("cstopb", 2, sys::CSTOPB),
    ("cread", 2, sys::CREAD),
    ("paren", 2, sys::PARENB),
    ("parodd", 2, sys::PARODD),
    ("hupcl", 2, sys::HUPCL),
    ("clocal", 2, sys::CLOCAL),
    ("isig", 3, sys::ISIG),
    ("icanon", 3, sys::ICANON),
    ("xcase", 3, sys::XCASE),
    ("echo", 3, sys::ECHO),
    ("echoe", 3, sys::ECHOE),
    ("echok", 3, sys::ECHOK),
    ("echonl", 3, sys::ECHONL),
    ("noflsh", 3, sys::NOFLSH),
    ("tostop", 3, sys::TOSTOP),
    ("iexten", 3, sys::IEXTEN),
];

const CCHARS: &[(&str, usize)] = &[
    ("VINTR", sys::VINTR),
    ("VQUIT", sys::VQUIT),
    ("VERASE", sys::VERASE),
    ("VKILL", sys::VKILL),
    ("VEOF", sys::VEOF),
    ("VTIME", sys::VTIME),
    ("VMIN", sys::VMIN),
    ("VSWTC", sys::VSWTC),
    ("VSTART", sys::VSTART),
    ("VSTOP", sys::VSTOP),
    ("VSUSP", sys::VSUSP),
    ("VEOL", sys::VEOL),
    ("VREPRINT", sys::VREPRINT),
    ("VDISCARD", sys::VDISCARD),
    ("VWERASE", sys::VWERASE),
    ("VLNEXT", sys::VLNEXT),
    ("VEOL2", sys::VEOL2),
];

fn flag_enabled(t: &sys::Termios, class: u8, bit: u32) -> bool {
    let word = match class {
        0 => t.c_iflag,
        1 => t.c_oflag,
        2 => t.c_cflag,
        _ => t.c_lflag,
    };
    if bit >= 900 {
        // CSIZE 群
        return match bit {
            999 => word & sys::CSIZE == sys::CS5,
            998 => word & sys::CSIZE == sys::CS6,
            997 => word & sys::CSIZE == sys::CS7,
            _ => word & sys::CSIZE == sys::CS8,
        };
    }
    word & bit != 0
}

fn set_flag(t: &mut sys::Termios, class: u8, bit: u32, on: bool) {
    let word = match class {
        0 => &mut t.c_iflag,
        1 => &mut t.c_oflag,
        2 => &mut t.c_cflag,
        _ => &mut t.c_lflag,
    };
    if bit >= 900 {
        let val = match bit {
            999 => sys::CS5,
            998 => sys::CS6,
            997 => sys::CS7,
            _ => sys::CS8,
        };
        if on {
            *word = (*word & !sys::CSIZE) | val;
        }
        return;
    }
    if on {
        *word |= bit;
    } else {
        *word &= !bit;
    }
}

fn print_all(t: &sys::Termios, stdout: Out) {
    writeln!(
        stdout,
        "speed {} baud; ospeed {} baud; line = {};",
        t.c_ispeed, t.c_ospeed, t.c_line
    )
    .ok();
    let mut parts: Vec<String> = Vec::new();
    for (name, class, bit) in FLAGS {
        if flag_enabled(t, *class, *bit) {
            parts.push(name.to_string());
        } else {
            parts.push(format!("-{}", name));
        }
    }
    writeln!(stdout, "{}", parts.join(" ")).ok();
    let mut cc: Vec<String> = Vec::new();
    for (name, idx) in CCHARS {
        cc.push(format!("{}=0x{:02X}", name, t.c_cc[*idx]));
    }
    writeln!(stdout, "cchars: {}", cc.join(" ")).ok();
}

fn print_save(t: &sys::Termios, stdout: Out) {
    let mut parts: Vec<String> = vec![
        format!("ispeed {}", t.c_ispeed),
        format!("ospeed {}", t.c_ospeed),
    ];
    for (name, class, bit) in FLAGS {
        if flag_enabled(t, *class, *bit) {
            parts.push(name.to_string());
        } else {
            parts.push(format!("-{}", name));
        }
    }
    for (name, idx) in CCHARS {
        parts.push(format!("{}=0x{:02X}", name, t.c_cc[*idx]));
    }
    writeln!(stdout, "{}", parts.join(" ")).ok();
}

fn apply_preset(t: &mut sys::Termios, preset: &str) {
    match preset {
        "raw" => {
            t.c_iflag &= !(sys::IGNBRK | sys::BRKINT | sys::PARMRK | sys::ISTRIP | sys::INLCR | sys::IGNCR | sys::ICRNL | sys::IXON);
            t.c_oflag &= !sys::OPOST;
            t.c_lflag &= !(sys::ECHONL | sys::ICANON | sys::ISIG | sys::IEXTEN | sys::ECHO | sys::ECHOE | sys::ECHOK);
            t.c_cflag &= !sys::PARENB;
            t.c_cflag |= sys::CS8 | sys::CLOCAL;
            t.c_cc[sys::VMIN] = 1;
            t.c_cc[sys::VTIME] = 0;
        }
        "cooked" | "sane" => {
            t.c_iflag |= sys::BRKINT | sys::ICRNL | sys::IXON | sys::IEXTEN;
            t.c_iflag &= !(sys::IGNBRK | sys::PARMRK | sys::ISTRIP | sys::INLCR | sys::IGNCR | sys::IXOFF | sys::IXANY);
            t.c_oflag |= sys::OPOST | sys::ONLCR;
            t.c_oflag &= !(sys::OCRNL | sys::ONOCR | sys::ONLRET);
            t.c_cflag |= sys::CS8 | sys::CREAD | sys::HUPCL;
            t.c_cflag &= !(sys::PARENB | sys::PARODD | sys::CSTOPB | sys::CLOCAL);
            t.c_lflag |= sys::ISIG | sys::ICANON | sys::ECHO | sys::ECHOE | sys::ECHOK | sys::IEXTEN;
            t.c_lflag &= !(sys::XCASE | sys::ECHONL | sys::NOFLSH | sys::TOSTOP);
            // sane 的 cchars 預設值（我方決定；VINTR ^C、VQUIT ^\、VERASE DEL 等）
            t.c_cc[sys::VINTR] = 0x03;
            t.c_cc[sys::VQUIT] = 0x1C;
            t.c_cc[sys::VERASE] = 0x7F;
            t.c_cc[sys::VKILL] = 0x15;
            t.c_cc[sys::VEOF] = 0x04;
            t.c_cc[sys::VTIME] = 0;
            t.c_cc[sys::VMIN] = 1;
            t.c_cc[sys::VSTART] = 0x11;
            t.c_cc[sys::VSTOP] = 0x13;
            t.c_cc[sys::VSUSP] = 0x1A;
            t.c_cc[sys::VEOL] = 0;
            t.c_cc[sys::VREPRINT] = 0x12;
            t.c_cc[sys::VDISCARD] = 0x16;
            t.c_cc[sys::VWERASE] = 0x17;
            t.c_cc[sys::VLNEXT] = 0x16;
            t.c_cc[sys::VEOL2] = 0;
        }
        _ => {}
    }
}

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut all = false;
    let mut save = false;
    let mut device: Option<String> = None;
    let mut settings: Vec<String> = Vec::new();
    let mut only_positional = false;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        i += 1;
        if only_positional {
            settings.push(a.clone());
            continue;
        }
        if a == "--" {
            only_positional = true;
            continue;
        }
        if a == "--help" || a == "-h" {
            write!(stdout, "{}", STTY_HELP).ok();
            return 0;
        }
        if a == "--version" || a == "-V" {
            writeln!(stdout, "stty (uutils coreutils) 0.10.0").ok();
            return 0;
        }
        if a.starts_with("--") {
            let name = a[2..].split('=').next().unwrap_or("").to_string();
            let value = a.split_once('=').map(|(_, v)| v.to_string());
            match pathname::resolve_long(&name, STTY_LONGS) {
                Ok("help") => {
                    if let Some(v) = value {
                        return pathname::clap_unexpected_value("help", &v, "stty", stderr);
                    }
                    write!(stdout, "{}", STTY_HELP).ok();
                    return 0;
                }
                Ok("version") => {
                    if let Some(v) = value {
                        return pathname::clap_unexpected_value("version", &v, "stty", stderr);
                    }
                    writeln!(stdout, "stty (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                Ok("all") => {
                    if let Some(v) = value {
                        return pathname::clap_unexpected_value("all", &v, "stty", stderr);
                    }
                    all = true;
                }
                Ok("save") => {
                    if let Some(v) = value {
                        return pathname::clap_unexpected_value("save", &v, "stty", stderr);
                    }
                    save = true;
                }
                Ok("file") => match value {
                    Some(v) => device = Some(v),
                    None => {
                        if i < args.len() {
                            device = Some(args[i].clone());
                            i += 1;
                        } else {
                            pathname::clap_value_required("file", "DEVICE", stderr);
                            return 2;
                        }
                    }
                },
                Ok(_) => return unexpected(a, stderr),
                Err(Some(c)) => {
                    write!(stderr, "error: unexpected argument '{}' found\n\n  tip: a similar argument exists: '--{}'\n\nUsage: stty [OPTION]... [SETTING]...\n\nFor more information, try '--help'.\n", a, c).ok();
                    return 2;
                }
                Err(None) => return unexpected(a, stderr),
            }
            continue;
        }
        // 「-echo」這類負面 settings（stty 慣例）優先於短旗標簇解析：
        // 只在剩余部分剛好是已知設定名時成立（否則照走 clap 短旗標路徑）。我方決定，見 assumptions。
        if a.starts_with('-') && a.len() > 2 && !a.starts_with("--") {
            let name = &a[1..];
            let known = FLAGS.iter().any(|(n, _, _)| *n == name)
                || matches!(name, "raw" | "cooked" | "sane" | "size" | "rows" | "cols" | "speed" | "ispeed" | "ospeed");
            if known {
                settings.push(a.clone());
                continue;
            }
        }
        if a.starts_with('-') && a.len() > 1 {
            let chars: Vec<char> = a[1..].chars().collect();
            let mut k = 0;
            while k < chars.len() {
                match chars[k] {
                    'a' => all = true,
                    'g' => save = true,
                    'F' => {
                        if k + 1 < chars.len() {
                            device = Some(chars[k + 1..].iter().collect());
                            break;
                        }
                        if i < args.len() {
                            device = Some(args[i].clone());
                            i += 1;
                        } else {
                            pathname::clap_value_required("file", "DEVICE", stderr);
                            return 2;
                        }
                    }
                    'V' => {
                        writeln!(stdout, "stty (uutils coreutils) 0.10.0").ok();
                        return 0;
                    }
                    _ => {
                        return unexpected(&format!("-{}", chars[k]), stderr);
                    }
                }
                k += 1;
            }
            continue;
        }
        settings.push(a.clone());
    }

    // 取得 fd：-F 開裝置，否則 stdin。
    let opened: Option<File>;
    let fd: i32;
    if let Some(dev) = &device {
        match File::options().read(true).write(true).open(dev) {
            Ok(f) => {
                fd = f.as_raw_fd();
                opened = Some(f);
            }
            Err(e) => return io_err(dev, &e, stderr),
        }
    } else {
        opened = None;
        fd = sys::STDIN_FD;
    }
    let _keep = opened;

    let mut t = match sys::termios_get(fd) {
        Ok(t) => t,
        Err(e) => {
            let label = if device.is_some() { device.clone().unwrap() } else { "standard input".to_string() };
            return io_err(&label, &e, stderr);
        }
    };

    let label = if device.is_some() { device.clone().unwrap() } else { "standard input".to_string() };

    // settings 套用
    let mut s = 0;
    while s < settings.len() {
        let tok = &settings[s];
        s += 1;
        if tok == "raw" || tok == "cooked" || tok == "sane" {
            apply_preset(&mut t, tok);
            continue;
        }
        if tok == "size" {
            match sys::tty_winsize(fd) {
                Some((r, c)) => {
                    writeln!(stdout, "{} {}", r, c).ok();
                }
                None => return io_err(&label, &std::io::Error::from_raw_os_error(25), stderr),
            }
            continue;
        }
        if tok == "rows" || tok == "cols" {
            let n = match settings.get(s) {
                Some(v) => v.clone(),
                None => {
                    writeln!(stderr, "stty: missing operand after '{}'", tok).ok();
                    return 1;
                }
            };
            s += 1;
            let n: u16 = match n.parse() {
                Ok(v) => v,
                Err(_) => {
                    writeln!(stderr, "stty: invalid argument '{}'", n).ok();
                    return 1;
                }
            };
            extern "C" {
                #[link_name = "ioctl"]
                fn c_ioctl(fd: i32, req: u64, ...) -> i32;
            }
            const TIOCSWINSZ: u64 = 0x5414;
            #[repr(C)]
            struct Winsize {
                rows: u16,
                cols: u16,
                x: u16,
                y: u16,
            }
            let (rows, cols) = sys::tty_winsize(fd).unwrap_or((0, 0));
            let ws = Winsize {
                rows: if tok == "rows" { n } else { rows },
                cols: if tok == "cols" { n } else { cols },
                x: 0,
                y: 0,
            };
            if unsafe { c_ioctl(fd, TIOCSWINSZ, &ws as *const Winsize) } != 0 {
                return io_err(&label, &std::io::Error::last_os_error(), stderr);
            }
            continue;
        }
        if tok == "speed" || tok == "ispeed" || tok == "ospeed" {
            let n = match settings.get(s) {
                Some(v) => v.clone(),
                None => {
                    writeln!(stderr, "stty: missing operand after '{}'", tok).ok();
                    return 1;
                }
            };
            s += 1;
            let n: u32 = match n.parse() {
                Ok(v) => v,
                Err(_) => {
                    writeln!(stderr, "stty: invalid baud rate '{}'", n).ok();
                    return 1;
                }
            };
            if tok == "speed" || tok == "ispeed" {
                t.c_ispeed = n;
            }
            if tok == "speed" || tok == "ospeed" {
                t.c_ospeed = n;
            }
            continue;
        }
        // cchars NAME=0xNN
        if let Some((name, value)) = tok.split_once('=') {
            match CCHARS.iter().find(|(n, _)| *n == name) {
                Some((_, idx)) => {
                    let v = if let Some(hex) = value.strip_prefix("0x") {
                        u8::from_str_radix(hex, 16)
                    } else {
                        value.parse::<u8>()
                    };
                    match v {
                        Ok(v) => t.c_cc[*idx] = v,
                        Err(_) => {
                            writeln!(stderr, "stty: invalid control character value '{}'", value).ok();
                            return 1;
                        }
                    }
                    continue;
                }
                None => {
                    writeln!(stderr, "stty: unknown setting '{}'", tok).ok();
                    return 1;
                }
            }
        }
        // 旗標（可帶前導 -）
        let (on, bare) = if let Some(rest) = tok.strip_prefix('-') {
            (false, rest.to_string())
        } else {
            (true, tok.clone())
        };
        match FLAGS.iter().find(|(n, _, _)| *n == bare.as_str()) {
            Some((_, class, bit)) => set_flag(&mut t, *class, *bit, on),
            None => {
                writeln!(stderr, "stty: unknown setting '{}'", tok).ok();
                return 1;
            }
        }
    }

    let modified = !settings.is_empty();
    if modified {
        if let Err(e) = sys::termios_set(fd, &t) {
            return io_err(&label, &e, stderr);
        }
    }

    if save {
        print_save(&t, stdout);
    } else if all {
        print_all(&t, stdout);
    } else if !modified {
        writeln!(stdout, "speed {} baud;", t.c_ospeed).ok();
    }
    0
}
