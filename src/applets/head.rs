
use crate::In;
use crate::Out;

fn parse_count(s: &str) -> Option<i64> {
    if s.starts_with('-') {
        s[1..].parse::<i64>().ok().map(|v| -v)
    } else {
        s.parse::<i64>().ok()
    }
}

/// head: print the first N lines (or bytes) of each FILE to stdout.
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
            writeln!(stderr, "Usage: head [OPTIONS] [FILE]").unwrap();
            return 0;
        }
        if a == "--version" || a == "-V" {
            writeln!(stderr, "head (coreutils)").unwrap();
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
        if a == "-n" || a == "--lines" {
            if i + 1 >= args.len() {
                writeln!(stderr, "head: missing argument after {a}").unwrap();
                return 2;
            }
            match parse_count(&args[i + 1]) {
                Some(v) => lines = v,
                None => {
                    writeln!(stderr, "head: invalid number: {}", args[i + 1]).unwrap();
                    return 2;
                }
            }
            i += 2;
            continue;
        }
        if a == "-c" || a == "--bytes" {
            if i + 1 >= args.len() {
                writeln!(stderr, "head: missing argument after {a}").unwrap();
                return 2;
            }
            match parse_count(&args[i + 1]) {
                Some(v) => bytes = Some(v),
                None => {
                    writeln!(stderr, "head: invalid number: {}", args[i + 1]).unwrap();
                    return 2;
                }
            }
            i += 2;
            continue;
        }
        if a.starts_with('-') && a != "-" {
            let n = parse_count(a);
            if n.is_some() {
                lines = n.unwrap();
                i += 1;
                continue;
            }
            writeln!(stderr, "head: unknown option {a}").unwrap();
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
                    writeln!(stderr, "head: {f}: {e}").unwrap();
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

fn emit(data: &[u8], lines: i64, bytes: Option<i64>, term: u8, stdout: Out) -> bool {
    if let Some(b) = bytes {
        let take = if b < 0 {
            data.len().saturating_sub(b.unsigned_abs() as usize)
        } else {
            (b as usize).min(data.len())
        };
        return stdout.write_all(&data[..take]).is_ok();
    }
    if lines < 0 {
        let keep = lines.unsigned_abs() as usize;
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
        let n = parts.len().saturating_sub(keep);
        for p in &parts[..n] {
            if stdout.write_all(p).is_err() {
                return false;
            }
        }
        return true;
    }
    let mut taken = 0;
    let mut start = 0;
    while taken < lines as usize && start < data.len() {
        match data[start..].iter().position(|&c| c == term) {
            Some(p) => {
                if stdout.write_all(&data[start..start + p + 1]).is_err() {
                    return false;
                }
                start += p + 1;
                taken += 1;
            }
            None => {
                if stdout.write_all(&data[start..]).is_err() {
                    return false;
                }
                break;
            }
        }
    }
    true
}
