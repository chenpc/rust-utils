//! stdbuf — 以調整過的 buffer 設定執行 COMMAND。
//!
//! 規格來源：`docs/man/stdbuf.1.gz`（OPTIONS：`-i/--input`、`-o/--output`、`-e/--error`、
//! `-h`、`-V`；EXTRA 定義 MODE：`L` = line buffered（stdin 不可用）、`0` = unbuffered、
//! 數字可接 `KB 1000, K 1024, MB 1000*1000, M 1024*1024, and so on for G, T, P, E, Z, Y`）。
//! CLI 層按 clap 措辭（缺值錯誤一律用長名稱 `'--output <MODE>'`，即使寫的是 `-o`）。
//!
//! 本專案的 binary 是靜態連結、guest 內沒有動態載入器，GNU/參考的 `LD_PRELOAD=libstdbuf.so`
//! 機制無法重現；差異記錄在 docs/assumptions.md（仍把設定以 `_STDBUF_I/_STDBUF_O/_STDBUF_E`
//! 傳給子行程）。

use crate::exec::{is_executable, resolve_program, status_code};
use crate::pathname;
use crate::{In, Out};

const STDBUF_HELP: &str = r#"Run COMMAND, with modified buffering operations for its standard streams.

Usage: stdbuf [OPTION]... COMMAND

Options:
  -i, --input <MODE>   adjust standard input stream buffering
  -o, --output <MODE>  adjust standard output stream buffering
  -e, --error <MODE>   adjust standard error stream buffering
  -h, --help           Print help
  -V, --version        Print version

If MODE is 'L' the corresponding stream will be line buffered.
This option is invalid with standard input.

If MODE is '0' the corresponding stream will be unbuffered.

Otherwise, MODE is a number which may be followed by one of the following:

KB 1000, K 1024, MB 1000*1000, M 1024*1024, and so on for G, T, P, E, Z, Y.
In this case the corresponding stream will be fully buffered with the buffer size set to MODE bytes.

NOTE: If COMMAND adjusts the buffering of its standard streams (tee does for e.g.) then that will
override corresponding settings changed by stdbuf.
Also some filters (like dd and cat etc.) don't use streams for I/O, and are thus unaffected by
stdbuf settings.
"#;

const STDBUF_USAGE: &str = "Usage: stdbuf [OPTION]... COMMAND";
const STDBUF_LONGS: &[&str] = &[
    "error",
    "help",
    "input",
    "output",
    "version",
];

#[derive(Clone, Copy)]
enum Mode {
    Line,
    Bytes(u64),
}

enum ModeErr {
    Invalid,
    TooLarge,
}

/// man 的 MODE 語法：`L`、`0`、或數字可接單字母（1024 冪）/ 大寫字母＋B（1000 冪）後綴。
/// 數字部分可省略（`K` 等同 `1K`）；`0x` 開頭走十六進位、`0b` 走二進位、開頭 0 走八進位
/// （與本專案其他 applet 的數字語法一致）。
fn parse_mode(mode: &str) -> Result<Mode, ModeErr> {
    if mode == "L" {
        return Ok(Mode::Line);
    }
    if mode.is_empty() {
        return Err(ModeErr::Invalid);
    }
    let (digits, suffix, radix) = if mode.starts_with("0x") && mode.len() > 2 {
        let rest = &mode[2..];
        let n = rest.bytes().take_while(|b| b.is_ascii_hexdigit()).count();
        // 實測 `0xg`/`0xk` 被拒：0x 開頭但沒有十六進位數字。
        if n == 0 {
            return Err(ModeErr::Invalid);
        }
        (&rest[..n], &rest[n..], 16)
    } else if mode.starts_with("0b") && mode.len() > 2 {
        let rest = &mode[2..];
        let n = rest.bytes().take_while(|b| *b == b'0' || *b == b'1').count();
        if n == 0 {
            return Err(ModeErr::Invalid);
        }
        (&rest[..n], &rest[n..], 2)
    } else {
        let n = mode.bytes().take_while(|b| b.is_ascii_digit()).count();
        if n > 1 && mode.starts_with('0') {
            // 開頭 0 = 八進位（實測 `012k` = 10240）。
            let digits = &mode[..n];
            if digits.bytes().any(|b| b > b'7') {
                return Err(ModeErr::Invalid);
            }
            (digits, &mode[n..], 8)
        } else {
            (&mode[..n], &mode[n..], 10)
        }
    };
    let mult = match multiplier(suffix) {
        Some(m) => m,
        None => return Err(ModeErr::Invalid),
    };
    if digits.is_empty() && suffix.is_empty() {
        return Err(ModeErr::Invalid);
    }
    let value: u128 = if digits.is_empty() {
        1
    } else {
        match u128::from_str_radix(digits, radix) {
            Ok(v) => v,
            Err(_) => return Err(ModeErr::TooLarge),
        }
    };
    if !suffix.is_empty() && digits.len() > 1 {
        // 實測：`00k`/`000k`/`00z`/`00m` 被拒（多位數字且值為 0）；
        // `0k`、`01k`（八進位 1）、`010k`（八進位 8）可。
        if let Ok(0) = u128::from_str_radix(digits, radix) {
            return Err(ModeErr::Invalid);
        }
    }
    match value.checked_mul(mult) {
        Some(t) if t <= u64::MAX as u128 => Ok(Mode::Bytes(t as u64)),
        _ => Err(ModeErr::TooLarge),
    }
}

fn multiplier(suffix: &str) -> Option<u128> {
    if suffix.is_empty() {
        return Some(1);
    }
    let bytes = suffix.as_bytes();
    if bytes.len() == 1 && bytes[0] == b'b' {
        return Some(512);
    }
    // 實測：單字元 k/m/g/t/p/e/z/y = 1024^n、b = 512；字元＋大写 B = 1000^n（`1kB` 可、`1Kb` 不可）。
    let letter = match bytes[0] {
        b'k' | b'K' => 1,
        b'm' | b'M' => 2,
        b'g' | b'G' => 3,
        b't' | b'T' => 4,
        b'p' | b'P' => 5,
        b'e' | b'E' => 6,
        b'z' | b'Z' => 7,
        b'y' | b'Y' => 8,
        _ => return None,
    };
    let base: u128 = if bytes.len() == 1 {
        1024
    } else if bytes.len() == 2 && bytes[1] == b'B' {
        1000
    } else {
        return None;
    };
    let mut v: u128 = 1;
    for _ in 0..letter {
        v = v.checked_mul(base)?;
    }
    Some(v)
}

fn trailing(
    stderr: Out,
    tip: Option<&str>,
    with_usage: bool,
) {
    writeln!(stderr).ok();
    if let Some(t) = tip {
        writeln!(stderr, "  tip: {}", t).ok();
    }
    if with_usage {
        writeln!(stderr).ok();
        writeln!(stderr, "{}", STDBUF_USAGE).ok();
    }
    writeln!(stderr).ok();
    writeln!(stderr, "For more information, try '--help'.").ok();
}

fn required(modes_given: bool, command_given: bool, stderr: Out) -> i32 {
    write!(
        stderr,
        "error: the following required arguments were not provided:\n"
    )
    .ok();
    if !modes_given {
        writeln!(stderr, "  --input <MODE>").ok();
        writeln!(stderr, "  --output <MODE>").ok();
        writeln!(stderr, "  --error <MODE>").ok();
    }
    if !command_given {
        writeln!(stderr, "  <command>...").ok();
    }
    writeln!(stderr).ok();
    writeln!(stderr, "{}", STDBUF_USAGE).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "For more information, try '--help'.").ok();
    125
}

fn unexpected(shown: &str, raw: &str, stderr: Out) -> i32 {
    writeln!(stderr, "error: unexpected argument '{}' found", shown).ok();
    trailing(
        stderr,
        Some(&format!("to pass '{}' as a value, use '-- {}'", raw, raw)),
        true,
    );
    125
}

fn value_required(long: &str, stderr: Out) -> i32 {
    pathname::clap_value_required(long, "MODE", stderr);
    125
}

fn mode_error(mode: &str, err: ModeErr, stderr: Out) -> i32 {
    // 實測：壞在「數字本身」的 MODE（`09`、`008`、`018`、`0123456789`、`00k`、`08k`、`0xg`）
    // REF 不加引號（純數字時還會去掉前導零）；壞在後綴結構的（`1kb`、`0kk`、`0b2`）才加引號。
    let (shown, quoted) = render_mode(mode);
    match err {
        ModeErr::Invalid => {
            writeln!(stderr, "stdbuf: invalid mode {}{}{}", quoted, shown, quoted).ok();
        }
        ModeErr::TooLarge => {
            writeln!(
                stderr,
                "stdbuf: invalid mode {}{}{}: Value too large for defined data type",
                quoted, shown, quoted
            )
            .ok();
        }
    }
    writeln!(stderr, "{}", pathname::usage_hint("stdbuf")).ok();
    125
}

/// `--x` / `-x` 這類「看起來是選項」的 token；回傳 clap 在錯誤裡顯示的字串。
fn displayed_unknown(token: &str) -> String {
    if token.starts_with("--") {
        token.to_string()
    } else {
        pathname::clap_unknown_short(token, 1)
    }
}

fn render_mode(mode: &str) -> (String, &'static str) {
    if !mode.starts_with('0') || mode.starts_with("0b") {
        return (mode.to_string(), "'");
    }
    if mode.starts_with("0x") {
        return (mode.to_string(), "");
    }
    let n = mode.bytes().take_while(|b| b.is_ascii_digit()).count();
    let suffix = &mode[n..];
    if suffix.is_empty() {
        let stripped = mode.trim_start_matches('0');
        return (if stripped.is_empty() { "0" } else { stripped }.to_string(), "");
    }
    if n >= 2 && multiplier(suffix).is_some() {
        return (mode.to_string(), "");
    }
    (mode.to_string(), "'")
}

fn is_known_long(token: &str) -> bool {
    let name = token[2..].split('=').next().unwrap_or("");
    matches!(pathname::resolve_long(name, STDBUF_LONGS), Ok(_))
}

fn is_known_short(token: &str) -> bool {
    matches!(
        token.as_bytes()[1],
        b'i' | b'o' | b'e' | b'h' | b'V'
    )
}

/// 這個 token 是否「不能被當成選項的值」（clap：任何看起來是選項的 token，包含未知的 `-1`/`-x`）。
fn not_a_value(token: &str) -> bool {
    token.starts_with('-') && token.len() > 1
}

struct Opts {
    input: Option<String>,
    output: Option<String>,
    error: Option<String>,
}

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut o = Opts {
        input: None,
        output: None,
        error: None,
    };
    let mut command: Option<String> = None;
    let mut cmd_args: Vec<String> = Vec::new();
    let mut only_positional = false;
    let mut i = 0;

    while i < args.len() {
        let a = &args[i];
        i += 1;
        if command.is_some() {
            cmd_args.push(a.clone());
            continue;
        }
        if only_positional {
            command = Some(a.clone());
            continue;
        }
        if a == "--" {
            only_positional = true;
            continue;
        }
        if a.starts_with("--") && a.len() > 2 {
            let name = a[2..].split('=').next().unwrap_or("").to_string();
            let attached = a[2..].split_once('=').map(|(_, v)| v.to_string());
            let long = match pathname::resolve_long(&name, STDBUF_LONGS) {
                Ok(l) => l,
                Err(_) => return unexpected(a, a, stderr),
            };
            match long {
                "help" => {
                    if let Some(v) = attached {
                        pathname::clap_unexpected_value("help", &v, &STDBUF_USAGE[7..], stderr);
                        return 125;
                    }
                    write!(stdout, "{}", STDBUF_HELP).ok();
                    return 0;
                }
                "version" => {
                    if let Some(v) = attached {
                        pathname::clap_unexpected_value("version", &v, &STDBUF_USAGE[7..], stderr);
                        return 125;
                    }
                    writeln!(stdout, "stdbuf (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                long => {
                    let value = match attached {
                        Some(v) => v,
                        None => {
                            match args.get(i) {
                                Some(next) if !not_a_value(next) => {
                                    let v = next.clone();
                                    i += 1;
                                    v
                                }
                                Some(next) => {
                                    if is_known_long(next) || is_known_short(next) || next == "--" {
                                        return value_required(long, stderr);
                                    }
                                    return unexpected(&displayed_unknown(next), next, stderr);
                                }
                                None => return value_required(long, stderr),
                            }
                        }
                    };
                    let slot = slot_for(&mut o, long);
                    if slot.is_some() {
                        return pathname::clap_dup(long, "MODE", 125, stderr);
                    }
                    *slot = Some(value);
                }
            }
            continue;
        }
        if a.starts_with('-') && a.len() > 1 {
            let mut k = 1;
            while k < a.len() {
                let long: &str = match a.as_bytes()[k] {
                    b'i' => "input",
                    b'o' => "output",
                    b'e' => "error",
                    b'h' => {
                        write!(stdout, "{}", STDBUF_HELP).ok();
                        return 0;
                    }
                    b'V' => {
                        writeln!(stdout, "stdbuf (uutils coreutils) 0.10.0").ok();
                        return 0;
                    }
                    _ => return unexpected(
                        &pathname::clap_unknown_short(a, k),
                        &a[..=k],
                        stderr,
                    ),
                };
                let rest = &a[k + 1..];
                let value: String;
                if !rest.is_empty() {
                    // 短旗標簇：第一個需要值的旗標吃掉剩下的字元（實測 `-oio 0` → mode 'io'、
                    // `-o=0` → mode '0'、`-o=` → mode ''）。
                    value = match rest.strip_prefix('=') {
                        Some(v) => v.to_string(),
                        None => rest.to_string(),
                    };
                    k = a.len();
                } else {
                    match args.get(i) {
                        Some(next) if !not_a_value(next) => {
                            value = next.clone();
                            i += 1;
                            k += 1;
                        }
                        Some(next) => {
                            if is_known_long(next) || is_known_short(next) || next == "--" {
                                return value_required(long, stderr);
                            }
                            return unexpected(&displayed_unknown(next), next, stderr);
                        }
                        None => return value_required(long, stderr),
                    }
                }
                let slot = slot_for(&mut o, long);
                if slot.is_some() {
                    return pathname::clap_dup(long, "MODE", 125, stderr);
                }
                *slot = Some(value);
            }
            continue;
        }
        command = Some(a.clone());
    }

    let modes_given = o.input.is_some() || o.output.is_some() || o.error.is_some();
    if !modes_given || command.is_none() {
        return required(modes_given, command.is_some(), stderr);
    }
    // 先查 `-i L`（實測 `-o bogus -i L` 也是先報 line buffering stdin）。
    if o.input.as_deref() == Some("L") {
        writeln!(stderr, "stdbuf: line buffering stdin is meaningless").ok();
        writeln!(stderr, "{}", pathname::usage_hint("stdbuf")).ok();
        return 125;
    }
    let mut parsed: Vec<(&'static str, Mode)> = Vec::new();
    for (name, slot) in [
        ("input", &o.input),
        ("output", &o.output),
        ("error", &o.error),
    ]
    .into_iter()
    {
        let Some(raw) = slot else { continue };
        match parse_mode(raw) {
            Ok(m) => parsed.push((name, m)),
            Err(e) => return mode_error(raw, e, stderr),
        }
    }

    let cmd = command.unwrap();
    let prog = match resolve_program(&cmd) {
        Some(p) => p,
        None => {
            writeln!(
                stderr,
                "stdbuf: failed to execute process: No such file or directory"
            )
            .ok();
            return 127;
        }
    };
    if !is_executable(&prog) {
        writeln!(stderr, "stdbuf: failed to execute process: Permission denied").ok();
        return 126;
    }
    let mut builder = std::process::Command::new(&prog);
    builder.args(&cmd_args);
    for (name, mode) in &parsed {
        let key = match *name {
            "input" => "_STDBUF_I",
            "output" => "_STDBUF_O",
            _ => "_STDBUF_E",
        };
        let value = match mode {
            Mode::Line => "L".to_string(),
            Mode::Bytes(b) => b.to_string(),
        };
        builder.env(key, value);
    }
    match builder.status() {
        Ok(status) => status_code(status),
        Err(e) => {
            writeln!(
                stderr,
                "stdbuf: failed to execute process: {}",
                crate::exec::errno_reason(&e)
            )
            .ok();
            if e.raw_os_error() == Some(13) {
                126
            } else {
                127
            }
        }
    }
}

fn slot_for<'a>(o: &'a mut Opts, long: &str) -> &'a mut Option<String> {
    match long {
        "input" => &mut o.input,
        "output" => &mut o.output,
        _ => &mut o.error,
    }
}
