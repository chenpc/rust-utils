//! pr — 分頁／分欄輸出檔案供列印。
//!
//! 規格來源：`docs/man/pr.1.gz`。man 未定義的部分我方決定（見 docs/assumptions.md「pr」節）：
//! - 五行頁首 = 日期頁碼行、標題行、3 個空行；五行頁尾 = 5 個空行。-F 時頁首 3 行、頁尾一個 \f（文字頁長 63）。
//! - 標題行：-h STRING 取代檔名；stdin 標題為 `-`。
//! - -D 日期格式預設 `%b %e %H:%M %Y`（我方 mini-strftime）。
//! - 頁碼每檔重新計算；多欄排版採「整檔先分欄、再依頁分列」。
//! - -n 行號寬度不足時截去左側數字（保留右側數字）。
//! - 輸入行內含 \f（且未給 -T）時該行輸出後強制換頁。
//! - --pages 跳過的頁不消耗 -n／-N 行號。

use std::fs::File;
use std::io::{BufRead, BufReader};

use crate::pathname;
use crate::sys;
use crate::{In, Out};

const PR_HELP: &str = r#"paginate or columnate FILE(s) for printing

Usage: pr [OPTION]... [FILE]...

Options:
      --pages <FIRST_PAGE[:LAST_PAGE]>  begin and stop printing with page FIRST_PAGE[:LAST_PAGE]
  -h, --header <STRING>                 use the string header to replace the file name in the header line
  -D, --date-format <FORMAT>            use 'date'-style FORMAT for the header date
  -d, --double-space                    produce output that is double spaced
  -n, --number-lines [<CHAR><WIDTH>]    provide width digit line numbering (default width 5, char TAB)
  -N, --first-line-number <NUMBER>      start counting with NUMBER at 1st line of first page printed
  -t, --omit-header                     write neither the five-line identifying header nor the five-line trailer
  -T, --omit-pagination                 omit page headers and trailers, eliminate pagination by form feeds
  -l, --length <PAGE_LENGTH>            override the 66-line default (default text 56, and with -F 63)
  -r, --no-file-warnings                omit warning when a file cannot be opened
  -F, --form-feed                       use a form-feed for new pages
  -w, --width <WIDTH>                   set the width for multiple text-column output only
  -W, --page-width <WIDTH>              set page width (72) always, truncate lines, except -J
  -a, --across                          fill columns across the page in round-robin order
      --column <COLUMN>                 produce multi-column output (default 1)
  -s, --separator [<CHAR>]              separate text columns by CHAR instead of by spaces (default TAB)
  -S, --sep-string [<STRING>]           separate columns by STRING (default TAB with -J, SPACE otherwise)
  -m, --merge                           merge files, one line from each side by side
  -o, --indent <MARGIN>                 each line of output preceded by MARGIN spaces
  -J                                    merge full lines, turns off -W line truncation, no column alignment
  -e, --expand-tabs [<CHAR><WIDTH>]     expand input CHARs (TABs) to tab WIDTH (8)
      --help                            print help
  -V, --version                         print version

Arguments:
  [files]  files to process (default: -)
"#;

const PR_LONGS: &[&str] = &[
    "across",
    "column",
    "date-format",
    "double-space",
    "expand-tabs",
    "first-line-number",
    "form-feed",
    "header",
    "help",
    "indent",
    "length",
    "merge",
    "no-file-warnings",
    "number-lines",
    "omit-header",
    "omit-pagination",
    "page-width",
    "pages",
    "sep-string",
    "separator",
    "version",
    "width",
];

#[derive(Default)]
struct Cfg {
    pages_first: usize,
    pages_last: usize,
    header: Option<String>,
    date_format: String,
    double_space: bool,
    number: Option<(Option<char>, usize)>,
    first_line_number: i64,
    omit_header: bool,
    omit_pagination: bool,
    page_length: usize,
    no_warnings: bool,
    form_feed: bool,
    width: Option<usize>,
    page_width: Option<usize>,
    across: bool,
    columns: usize,
    sep_char: Option<char>,
    sep_string: Option<String>,
    merge: bool,
    indent: usize,
    join_full: bool,
    expand: Option<(char, usize)>,
    files: Vec<String>,
}

fn usage_err(stderr: Out) -> i32 {
    write!(
        stderr,
        "Usage: pr [OPTION]... [FILE]...\n\nFor more information, try '--help'.\n"
    )
    .ok();
    2
}

fn unexpected(arg: &str, stderr: Out) -> i32 {
    write!(stderr, "error: unexpected argument '{}' found\n\n", arg).ok();
    usage_err(stderr);
    2
}

fn num(s: &str) -> Option<usize> {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) {
        s.parse().ok()
    } else {
        None
    }
}

// -n 的 [<CHAR><WIDTH>]：先吃非數字字元當 char，其餘當寬度（預設 5）。
fn parse_char_width(v: &str) -> (Option<char>, usize) {
    let mut ch: Option<char> = None;
    let mut digits = String::new();
    for c in v.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
        } else if ch.is_none() {
            ch = Some(c);
        }
    }
    let width = if digits.is_empty() { 5 } else { digits.parse().unwrap_or(5) };
    (ch, width)
}

// -e 的 [<CHAR><WIDTH>]：預設 TAB、寬 8。
fn parse_expand(v: &str) -> (char, usize) {
    let mut ch: Option<char> = None;
    let mut digits = String::new();
    for c in v.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
        } else if ch.is_none() {
            ch = Some(c);
        }
    }
    let width = if digits.is_empty() { 8 } else { digits.parse().unwrap_or(8) };
    (ch.unwrap_or('\t'), width)
}

fn header_date(fmt: &str) -> String {
    let now = sys::now_epoch();
    let tm = match sys::local_tm(now) {
        Some(t) => t,
        None => return String::new(),
    };
    let year = tm.tm_year + 1900;
    let mon = (tm.tm_mon + 1) as usize;
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let mut out = String::new();
    let mut it = fmt.chars().peekable();
    while let Some(c) = it.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('b') => out.push_str(MONTHS[(mon - 1) % 12]),
            Some('d') => out.push_str(&format!("{:02}", tm.tm_mday)),
            Some('e') => out.push_str(&format!("{:>2}", tm.tm_mday)),
            Some('H') => out.push_str(&format!("{:02}", tm.tm_hour)),
            Some('M') => out.push_str(&format!("{:02}", tm.tm_min)),
            Some('m') => out.push_str(&format!("{:02}", mon)),
            Some('Y') => out.push_str(&format!("{}", year)),
            Some('%') => out.push('%'),
            Some(o) => {
                out.push('%');
                out.push(o);
            }
            None => out.push('%'),
        }
    }
    out
}

fn expand_tabs_line(s: &str, tab_char: char, width: usize) -> String {
    if !s.contains(tab_char) {
        return s.to_string();
    }
    let mut out = String::new();
    let mut col = 0usize;
    for ch in s.chars() {
        if ch == tab_char {
            let step = if width == 0 { 0 } else { width - (col % width) };
            out.push_str(&" ".repeat(step));
            col += step;
        } else {
            out.push(ch);
            col += 1;
        }
    }
    out
}

fn char_count(s: &str) -> usize {
    s.chars().count()
}

fn truncate(s: &str, w: usize) -> String {
    if char_count(s) <= w {
        s.to_string()
    } else {
        s.chars().take(w).collect()
    }
}

fn pad(s: &str, w: usize) -> String {
    let n = char_count(s);
    if n >= w {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(w - n))
    }
}

fn read_lines(name: &str, stdin: In, cfg: &Cfg, stderr: Out) -> Option<Vec<String>> {
    if name == "-" {
        let mut v = Vec::new();
        let mut buf: Vec<u8> = Vec::new();
        let mut byte: [u8; 1] = [0];
        loop {
            match std::io::Read::read(stdin, &mut byte) {
                Ok(0) => {
                    if !buf.is_empty() {
                        v.push(String::from_utf8_lossy(&buf).into_owned());
                    }
                    break;
                }
                Ok(_) => {
                    if byte[0] == b'\n' {
                        v.push(String::from_utf8_lossy(&buf).into_owned());
                        buf.clear();
                    } else {
                        buf.push(byte[0]);
                    }
                }
                Err(_) => break,
            }
        }
        Some(v)
    } else {
        match File::open(name) {
            Ok(f) => {
                let reader = BufReader::new(f);
                let mut v = Vec::new();
                for line in reader.lines() {
                    match line {
                        Ok(l) => v.push(l),
                        Err(_) => break,
                    }
                }
                Some(v)
            }
            Err(e) => {
                if !cfg.no_warnings {
                    writeln!(stderr, "pr: {}: {}", name, pathname::io_reason_nosuffix(&e)).ok();
                }
                None
            }
        }
    }
}

fn apply_numbering(cfg: &Cfg, counter: &mut i64, text: String) -> String {
    if let Some((sep, width)) = &cfg.number {
        let n = counter.to_string();
        let shown = if n.chars().count() > *width {
            n.chars().skip(n.chars().count() - *width).collect::<String>()
        } else {
            format!("{}{}", " ".repeat(*width - n.chars().count()), n)
        };
        *counter += 1;
        match sep {
            Some(c) => format!("{}{}{}", shown, c, text),
            None => format!("{}\t{}", shown, text),
        }
    } else {
        text
    }
}

fn page_model(cfg: &Cfg) -> (usize, usize, usize) {
    // (header_lines, trailer_lines, text_per_page)
    if cfg.omit_header || cfg.omit_pagination {
        (0, 0, cfg.page_length)
    } else if cfg.form_feed {
        if cfg.page_length <= 4 {
            (0, 0, cfg.page_length)
        } else {
            (3, 1, cfg.page_length - 4)
        }
    } else if cfg.page_length <= 10 {
        (0, 0, cfg.page_length)
    } else {
        (5, 5, cfg.page_length - 10)
    }
}

fn emit_page(cfg: &Cfg, title: &str, page: usize, rows: &[Vec<String>], stdout: Out) {
    let (header_lines, trailer_lines, _tp) = page_model(cfg);
    let indent_s = " ".repeat(cfg.indent);
    if header_lines > 0 {
        writeln!(stdout, "{}{}  (page {})", indent_s, header_date(&cfg.date_format), page).ok();
        writeln!(stdout, "{}{}", indent_s, title).ok();
        for _ in 3..=header_lines {
            writeln!(stdout, "{}", indent_s).ok();
        }
    }
    let sep_char = cfg.sep_char.map(|c| c.to_string());
    for row in rows {
        // man：-S「no effect on column options」→ 多欄時 sep_string 不套用；-J（join_full）預設 TAB
        let sep: Option<String> = if row.len() > 1 {
            sep_char.clone()
        } else if cfg.join_full {
            Some("\t".to_string())
        } else {
            cfg.sep_string.clone()
        };
        let line = match &sep {
            Some(s) => row.join(s),
            None => row.join(""),
        };
        if cfg.double_space {
            writeln!(stdout, "{}{}", indent_s, line).ok();
            writeln!(stdout, "{}", indent_s).ok();
        } else {
            writeln!(stdout, "{}{}", indent_s, line).ok();
        }
    }
    if trailer_lines > 0 {
        if cfg.form_feed {
            write!(stdout, "\u{0C}").ok();
        } else {
            for _ in 0..trailer_lines {
                writeln!(stdout, "{}", indent_s).ok();
            }
        }
    }
}

fn render(cfg: &Cfg, lines: &[String], title: &str, stdout: Out) {
    let (_hl, _tl, text_per_page) = page_model(cfg);

    // 1) 预处理：-e 展開、（多欄時）-n 編號、欄寬截斷
    let mut cells: Vec<String> = lines
        .iter()
        .map(|l| match cfg.expand {
            Some((c, w)) => expand_tabs_line(l, c, w),
            None => l.clone(),
        })
        .collect();

    let multi = cfg.columns > 1;
    let mut counter = cfg.first_line_number;
    if !multi && cfg.number.is_some() {
        // 單欄：編號在 -W 截斷之前套到行首（編號占用前 width 個欄位）
        cells = cells.into_iter().map(|c| apply_numbering(cfg, &mut counter, c)).collect();
    }
    if !multi && !cfg.join_full {
        let pw = cfg.page_width.unwrap_or(72);
        cells = cells.into_iter().map(|c| truncate(&c, pw)).collect();
    }

    let col_width: usize = if multi {
        let base = cfg.width.unwrap_or(if cfg.sep_char.is_some() { 512 } else { 72 });
        let sep_total = cfg.columns - 1;
        let cw = if base > sep_total { (base - sep_total) / cfg.columns } else { 1 };
        match cfg.page_width {
            Some(pw) if !cfg.join_full => cw.min(pw),
            _ => cw,
        }
    } else {
        0
    };

    // 2) 組列（rows）
    let rows: Vec<Vec<String>> = if multi {
        if cfg.number.is_some() {
            cells = cells.into_iter().map(|c| apply_numbering(cfg, &mut counter, c)).collect();
        }
        let total = cells.len();
        let ncols = cfg.columns;
        let col_len = (total + ncols - 1) / ncols;
        let mut cols: Vec<Vec<String>> = vec![Vec::new(); ncols];
        for (i, cell) in cells.into_iter().enumerate() {
            let (ci, ri) = if cfg.across { (i % ncols, i / ncols) } else { (i / col_len, i % col_len) };
            if ci < ncols && ri < col_len {
                cols[ci].push(pad(&truncate(&cell, col_width), col_width));
            }
        }
        let mut rows: Vec<Vec<String>> = Vec::new();
        for r in 0..col_len {
            let mut row: Vec<String> = Vec::new();
            for c in &cols {
                row.push(c.get(r).cloned().unwrap_or_default());
            }
            rows.push(row);
        }
        rows
    } else {
        cells.into_iter().map(|c| vec![c]).collect()
    };

    // 3) \f 強制換頁（未給 -T 時）
    let force_break: Vec<bool> = if !cfg.omit_pagination {
        rows.iter().map(|r| r.last().map(|l| l.ends_with('\u{0C}')).unwrap_or(false)).collect()
    } else {
        vec![false; rows.len()]
    };

    // 4) 逐頁輸出（--pages 之外的頁只消耗不印）
    let mut page_idx = 0usize;
    let mut buf: Vec<Vec<String>> = Vec::new();
    for (i, row) in rows.into_iter().enumerate() {
        buf.push(row);
        if force_break[i] || buf.len() >= text_per_page {
            page_idx += 1;
            if page_idx >= cfg.pages_first && page_idx <= cfg.pages_last {
                emit_page(cfg, title, page_idx, &buf, &mut *stdout);
            }
            buf.clear();
        }
    }
    if !buf.is_empty() {
        page_idx += 1;
        if page_idx >= cfg.pages_first && page_idx <= cfg.pages_last {
            emit_page(cfg, title, page_idx, &buf, &mut *stdout);
        }
    }
}

fn merge_lines(cfg: &Cfg, readers: &[Vec<String>]) -> Vec<String> {
    let width = cfg.width.unwrap_or_else(|| cfg.page_width.unwrap_or(72));
    let k = readers.len().max(1);
    let col_w = if cfg.join_full { usize::MAX } else { (width / k).max(1) };
    let max_len = readers.iter().map(|r| r.len()).max().unwrap_or(0);
    let sep: String = match (&cfg.sep_string, &cfg.sep_char) {
        (Some(s), _) => s.clone(),
        (None, Some(c)) => c.to_string(),
        (None, None) => if cfg.join_full { "\t".to_string() } else { String::new() },
    };
    let mut merged: Vec<String> = Vec::new();
    for r in 0..max_len {
        let cells: Vec<String> = readers
            .iter()
            .map(|lines| {
                let cell = lines.get(r).cloned().unwrap_or_default();
                let cell = match cfg.expand {
                    Some((c, w)) => expand_tabs_line(&cell, c, w),
                    None => cell,
                };
                if cfg.join_full {
                    cell
                } else {
                    pad(&truncate(&cell, col_w), col_w)
                }
            })
            .collect();
        merged.push(cells.join(&sep));
    }
    merged
}

pub fn run(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut cfg = Cfg::default();
    cfg.pages_last = usize::MAX;
    cfg.page_length = 66;
    cfg.first_line_number = 1;
    cfg.date_format = "%b %e %H:%M %Y".to_string();
    let mut only_positional = false;
    let mut seen_long: Vec<&str> = Vec::new();
    let mut i = 0;

    while i < args.len() {
        let a = args[i].clone();
        i += 1;
        if only_positional {
            cfg.files.push(a);
            continue;
        }
        if a == "--" {
            only_positional = true;
            continue;
        }
        if a.starts_with("--") {
            let name = a[2..].split('=').next().unwrap_or("").to_string();
            let inline = a.split_once('=').map(|(_, v)| v.to_string());
            let resolved = match pathname::resolve_long(&name, PR_LONGS) {
                Ok(l) => l,
                Err(Some(c)) => {
                    write!(stderr, "error: unexpected argument '{}' found\n\n  tip: a similar argument exists: '--{}'\n\n", a, c).ok();
                    return usage_err(stderr);
                }
                Err(None) => return unexpected(&a, stderr),
            };
            if resolved == "help" {
                if inline.is_some() {
                    return pathname::clap_unexpected_value("help", &inline.unwrap(), "pr", stderr);
                }
                write!(stdout, "{}", PR_HELP).ok();
                return 0;
            }
            if resolved == "version" {
                if inline.is_some() {
                    return pathname::clap_unexpected_value("version", &inline.unwrap(), "pr", stderr);
                }
                writeln!(stdout, "pr (uutils coreutils) 0.10.0").ok();
                return 0;
            }
            if seen_long.contains(&resolved) {
                pathname::clap_dup(resolved, "", 2, stderr);
                return 2;
            }
            seen_long.push(resolved);
            // 需要值的長選項
            let metavar = match resolved {
                "column" => "COLUMN",
                "pages" => "FIRST_PAGE[:LAST_PAGE]",
                "header" => "STRING",
                "date-format" => "FORMAT",
                "first-line-number" => "NUMBER",
                "length" => "PAGE_LENGTH",
                "width" => "WIDTH",
                "page-width" => "PAGE_WIDTH",
                "indent" => "MARGIN",
                _ => "",
            };
            let takes_value = !metavar.is_empty();
            let optional_value = matches!(resolved, "separator" | "sep-string" | "number-lines" | "expand-tabs");
            if inline.is_some() && !takes_value && !optional_value {
                pathname::clap_unexpected_value(resolved, &inline.unwrap(), "pr [OPTION]... [FILE]...", stderr);
                return 2;
            }
            let value: Option<String> = if let Some(v) = inline {
                Some(v)
            } else if takes_value {
                if i < args.len() {
                    let v = args[i].clone();
                    i += 1;
                    Some(v)
                } else {
                    pathname::clap_value_required(resolved, metavar, stderr);
                    return 2;
                }
            } else if optional_value {
                if i < args.len() && !args[i].starts_with('-') {
                    let v = args[i].clone();
                    i += 1;
                    Some(v)
                } else {
                    None
                }
            } else {
                None
            };
            let bad = |what: &str, v: &str, stderr: Out| -> i32 {
                writeln!(stderr, "pr: invalid {}: '{}'", what, v).ok();
                1
            };
            match resolved {
                "across" => cfg.across = true,
                "double-space" => cfg.double_space = true,
                "form-feed" => cfg.form_feed = true,
                "merge" => cfg.merge = true,
                "omit-header" => cfg.omit_header = true,
                "omit-pagination" => cfg.omit_pagination = true,
                "no-file-warnings" => cfg.no_warnings = true,
                "column" => match num(value.as_deref().unwrap_or("")) {
                    Some(n) if n >= 1 => cfg.columns = n,
                    _ => return bad("number of columns", value.as_deref().unwrap_or(""), stderr),
                },
                "pages" => {
                    let v = value.unwrap_or_default();
                    let (f, l) = v.split_once(':').unwrap_or((v.as_str(), ""));
                    let lf = if l.is_empty() { Some(usize::MAX) } else { num(l) };
                    match (num(f), lf) {
                        (Some(f), Some(l)) if f >= 1 && (l >= f || l == usize::MAX) => {
                            cfg.pages_first = f;
                            cfg.pages_last = l;
                        }
                        _ => return bad("page range", &v, stderr),
                    }
                }
                "header" => cfg.header = Some(value.unwrap_or_default()),
                "date-format" => cfg.date_format = value.unwrap_or_default(),
                "first-line-number" => {
                    let v = value.unwrap_or_default();
                    match v.parse::<i64>() {
                        Ok(n) => cfg.first_line_number = n,
                        Err(_) => return bad("number", &v, stderr),
                    }
                }
                "length" => match num(value.as_deref().unwrap_or("")) {
                    Some(n) if n >= 1 => cfg.page_length = n,
                    _ => return bad("page length", value.as_deref().unwrap_or(""), stderr),
                },
                "width" => match num(value.as_deref().unwrap_or("")) {
                    Some(n) if n >= 1 => cfg.width = Some(n),
                    _ => return bad("width", value.as_deref().unwrap_or(""), stderr),
                },
                "page-width" => match num(value.as_deref().unwrap_or("")) {
                    Some(n) if n >= 1 => cfg.page_width = Some(n),
                    _ => return bad("page width", value.as_deref().unwrap_or(""), stderr),
                },
                "indent" => match num(value.as_deref().unwrap_or("")) {
                    Some(n) => cfg.indent = n,
                    None => return bad("margin", value.as_deref().unwrap_or(""), stderr),
                },
                "separator" => cfg.sep_char = Some(value.unwrap_or_else(|| "\t".to_string()).chars().next().unwrap_or('\t')),
                "sep-string" => cfg.sep_string = Some(value.unwrap_or_else(|| " ".to_string())),
                "number-lines" => cfg.number = Some(parse_char_width(value.as_deref().unwrap_or(""))),
                "expand-tabs" => cfg.expand = Some(parse_expand(value.as_deref().unwrap_or(""))),
                _ => return unexpected(&a, stderr),
            }
            continue;
        }
        if a.starts_with('-') && a.len() > 1 {
            let chars: Vec<char> = a[1..].chars().collect();
            let mut k = 0;
            while k < chars.len() {
                let c = chars[k];
                match c {
                    'a' => cfg.across = true,
                    'd' => cfg.double_space = true,
                    't' => cfg.omit_header = true,
                    'T' => cfg.omit_pagination = true,
                    'r' => cfg.no_warnings = true,
                    'F' => cfg.form_feed = true,
                    'm' => cfg.merge = true,
                    'J' => cfg.join_full = true,
                    'V' => {
                        writeln!(stdout, "pr (uutils coreutils) 0.10.0").ok();
                        return 0;
                    }
                    'l' | 'w' | 'W' | 'o' | 'N' | 'D' | 's' | 'S' | 'n' | 'e' | 'h' => {
                        // 值可附在簇內剩餘字元，或吃下一個 arg。
                        let rest: String = chars[k + 1..].iter().collect();
                        let optional = matches!(c, 's' | 'S' | 'n' | 'e');
                        let value: Option<String> = if !rest.is_empty() {
                            Some(rest)
                        } else if optional && (i >= args.len() || args[i].starts_with('-')) {
                            None
                        } else if i < args.len() {
                            let v = args[i].clone();
                            i += 1;
                            Some(v)
                        } else if optional {
                            None
                        } else {
                            let long = match c {
                                'l' => "length",
                                'w' => "width",
                                'W' => "page-width",
                                'o' => "indent",
                                'N' => "first-line-number",
                                'h' => "header",
                                _ => "date-format",
                            };
                            pathname::clap_value_required(long, "NUMBER", stderr);
                            return 2;
                        };
                        match c {
                            'l' => match num(value.as_deref().unwrap_or("")) {
                                Some(n) if n >= 1 => cfg.page_length = n,
                                _ => return bad_num(stderr, "page length", value.as_deref().unwrap_or("")),
                            },
                            'w' => match num(value.as_deref().unwrap_or("")) {
                                Some(n) if n >= 1 => cfg.width = Some(n),
                                _ => return bad_num(stderr, "width", value.as_deref().unwrap_or("")),
                            },
                            'W' => match num(value.as_deref().unwrap_or("")) {
                                Some(n) if n >= 1 => cfg.page_width = Some(n),
                                _ => return bad_num(stderr, "page width", value.as_deref().unwrap_or("")),
                            },
                            'o' => match num(value.as_deref().unwrap_or("")) {
                                Some(n) => cfg.indent = n,
                                None => return bad_num(stderr, "margin", value.as_deref().unwrap_or("")),
                            },
                            'N' => {
                                let v = value.unwrap_or_default();
                                match v.parse::<i64>() {
                                    Ok(n) => cfg.first_line_number = n,
                                    Err(_) => return bad_num(stderr, "number", &v),
                                }
                            }
                            'D' => cfg.date_format = value.unwrap_or_default(),
                            's' => cfg.sep_char = Some(value.unwrap_or_else(|| "\t".to_string()).chars().next().unwrap_or('\t')),
                            'S' => cfg.sep_string = Some(value.unwrap_or_else(|| " ".to_string())),
                            'n' => cfg.number = Some(parse_char_width(value.as_deref().unwrap_or(""))),
                            'h' => cfg.header = Some(value.unwrap_or_default()),
                            _ => cfg.expand = Some(parse_expand(value.as_deref().unwrap_or(""))),
                        }
                        break;
                    }
                    _ => {
                        return unexpected(&format!("-{}", c), stderr);
                    }
                }
                k += 1;
            }
            continue;
        }
        cfg.files.push(a);
    }

    let files: Vec<String> = if cfg.files.is_empty() {
        vec!["-".to_string()]
    } else {
        cfg.files.clone()
    };

    let mut rc = 0;
    let mut per_file: Vec<(String, Vec<String>)> = Vec::new();
    for f in &files {
        match read_lines(f, stdin, &cfg, &mut *stderr) {
            Some(l) => per_file.push((f.clone(), l)),
            None => rc = 1,
        }
    }

    if cfg.merge {
        let readers: Vec<Vec<String>> = per_file.iter().map(|(_, l)| l.clone()).collect();
        let merged = merge_lines(&cfg, &readers);
        let title = cfg.header.clone().unwrap_or_else(|| files.join(" "));
        render(&cfg, &merged, &title, stdout);
    } else {
        for (f, lines) in &per_file {
            let title = cfg.header.clone().unwrap_or_else(|| f.clone());
            render(&cfg, lines, &title, stdout);
        }
    }
    rc
}

fn bad_num(stderr: Out, what: &str, v: &str) -> i32 {
    writeln!(stderr, "pr: invalid {}: '{}'", what, v).ok();
    1
}

