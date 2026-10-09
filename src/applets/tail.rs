
use crate::In;
use crate::Out;

/// tail: print the last N lines (or bytes) of each FILE to stdout.
///
/// `-n K` last K lines; `-n +K` from the Kth line; `-n -K` all but the first K
/// lines. Same convention for `-c`. `-f` is accepted but performs a single pass
/// (no polling loop) — see docs/assumptions.md.
pub fn run(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut lines: i64 = 10;
    let mut bytes: Option<i64> = None;
    let mut quiet = false;
    let mut verbose = false;
    let mut term: u8 = b'\n';
    let mut files: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--help" || a == "-h" {
            writeln!(stderr, "Usage: tail [OPTIONS] [FILE]").unwrap();
            return 0;
        }
        if a == "--version" || a == "-V" {
            writeln!(stderr, "tail (coreutils)").unwrap();
            return 0;
        }
        if a == "-q" || a == "--quiet" {
            quiet = true;
            i += 1;
            continue;
        }
        if a == "-v" || a == "--verbose" {
            verbose = true;
            i += 1;
            continue;
        }
        if a == "-z" || a == "--zero-terminated" {
            term = 0;
            i += 1;
            continue;
        }
        if a == "-f" || a == "--follow" || a == "-F" {
            i += 1;
            continue;
        }
        if a == "-s" || a == "--sleep-interval" {
            i += 2;
            continue;
        }
        if a == "-n" || a == "--lines" {
            if i + 1 >= args.len() {
                writeln!(stderr, "tail: missing argument after {a}").unwrap();
                return 2;
            }
            match parse_count(&args[i + 1]) {
                Some(v) => lines = v,
                None => {
                    writeln!(stderr, "tail: invalid number: {}", args[i + 1]).unwrap();
                    return 2;
                }
            }
            i += 2;
            continue;
        }
        if a == "-c" || a == "--bytes" {
            if i + 1 >= args.len() {
                writeln!(stderr, "tail: missing argument after {a}").unwrap();
                return 2;
            }
            match parse_count(&args[i + 1]) {
                Some(v) => bytes = Some(v),
                None => {
                    writeln!(stderr, "tail: invalid number: {}", args[i + 1]).unwrap();
                    return 2;
                }
            }
            i += 2;
            continue;
        }
        if a.starts_with('-') && a != "-" {
            writeln!(stderr, "tail: unknown option {a}").unwrap();
            return 2;
        }
        files.push(a.clone());
        i += 1;
    }

    if files.is_empty() {
        let mut data = Vec::new();
        if stdin.read_to_end(&mut data).is_err() {
            return 1;
        }
        if !emit(&data, lines, bytes, term, stdout) {
            return 1;
        }
        return 0;
    }

    let show_header = verbose || (!quiet && files.len() > 1);
    let mut status = 0;
    for f in &files {
        let data = if f == "-" {
            let mut d = Vec::new();
            if stdin.read_to_end(&mut d).is_err() {
                return 1;
            }
            d
        } else {
            match std::fs::read(f) {
                Ok(d) => d,
                Err(e) => {
                    writeln!(stderr, "tail: {f}: {e}").unwrap();
                    status = 1;
                    continue;
                }
            }
        };
        if show_header {
            if stdout.write_all(b"==> ").is_err() {
                return 1;
            }
            if stdout.write_all(f.as_bytes()).is_err() {
                return 1;
            }
            if stdout.write_all(b" <==\n").is_err() {
                return 1;
            }
        }
        if !emit(&data, lines, bytes, term, stdout) {
            return 1;
        }
    }
    status
}

fn parse_count(s: &str) -> Option<i64> {
    if let Some(rest) = s.strip_prefix('+') {
        // from the Kth line == skip the first K-1
        return rest.parse::<i64>().ok().map(|v| -(v - 1));
    }
    if let Some(rest) = s.strip_prefix('-') {
        return rest.parse::<i64>().ok().map(|v| -v);
    }
    s.parse::<i64>().ok()
}

fn emit(data: &[u8], lines: i64, bytes: Option<i64>, term: u8, stdout: Out) -> bool {
    if let Some(b) = bytes {
        let slice = if b < 0 {
            (b.unsigned_abs() as usize).min(data.len())
        } else {
            data.len().saturating_sub(b as usize)
        };
        return stdout.write_all(&data[slice..]).is_ok();
    }
    let mut parts: Vec<&[u8]> = Vec::new();
    let mut start = 0;
    while start < data.len() {
        match data[start..].iter().position(|&c| c == term) {
            Some(p) => {
                parts.push(&data[start..start + p + 1]);
                start += p + 1;
            }
            None => {
                parts.push(&data[start..]);
                break;
            }
        }
    }
    let from = if lines < 0 {
        lines.unsigned_abs() as usize
    } else {
        parts.len().saturating_sub(lines as usize)
    };
    for p in parts.iter().skip(from.min(parts.len())) {
        if stdout.write_all(p).is_err() {
            return false;
        }
    }
    true
}
