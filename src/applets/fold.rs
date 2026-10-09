
use crate::{In, Out};

pub fn run(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut width = 80usize;
    let mut by_chars = true;
    let mut spaces = false;
    let mut files: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--help" {
            writeln!(stderr, "Usage: fold [OPTIONS] [FILE]...").unwrap();
            return 0;
        }
        if a == "--version" {
            writeln!(stderr, "fold (coreutils)").unwrap();
            return 0;
        }
        if a.starts_with("--") {
            let name = &a[2..];
            let mut taken = false;
            match name {
                "bytes" => by_chars = false,
                "spaces" => spaces = true,
                "width" => {
                    if i + 1 >= args.len() {
                        writeln!(stderr, "fold: missing argument after --width").unwrap();
                        return 2;
                    }
                    width = args[i + 1].parse().unwrap_or(80);
                    taken = true;
                }
                _ => {
                    writeln!(stderr, "fold: unknown option --{name}").unwrap();
                    return 2;
                }
            }
            i += if taken { 2 } else { 1 };
            continue;
        }
        if a.starts_with('-') && a.len() > 1 {
            let mut taken = false;
            for c in a[1..].chars() {
                match c {
                    'b' => by_chars = false,
                    'c' => by_chars = true,
                    's' => spaces = true,
                    'w' => {
                        if i + 1 >= args.len() {
                            writeln!(stderr, "fold: missing argument after -w").unwrap();
                            return 2;
                        }
                        width = args[i + 1].parse().unwrap_or(80);
                        taken = true;
                    }
                    _ => {
                        writeln!(stderr, "fold: unknown option -{c}").unwrap();
                        return 2;
                    }
                }
            }
            i += if taken { 2 } else { 1 };
            continue;
        }
        files.push(a.clone());
        i += 1;
    }

    if width == 0 {
        writeln!(stderr, "fold: invalid width: 0").unwrap();
        return 2;
    }

    let sources: Vec<String> = if files.is_empty() {
        vec!["-".to_string()]
    } else {
        files
    };
    let mut status = 0;

    for f in sources {
        let data = if f == "-" {
            let mut b = Vec::new();
            if stdin.read_to_end(&mut b).is_err() {
                status = 1;
                continue;
            }
            b
        } else {
            match std::fs::read(&f) {
                Ok(b) => b,
                Err(e) => {
                    writeln!(stderr, "fold: {f}: {e}").unwrap();
                    status = 1;
                    continue;
                }
            }
        };

        let parts: Vec<&[u8]> = data.split(|b| *b == b'\n').collect();
        let trailing = data.last() == Some(&b'\n');
        for (pi, raw) in parts.iter().enumerate() {
            if raw.is_empty() && trailing && pi == parts.len() - 1 {
                continue;
            }
            let mut pos = 0usize;
            while pos < raw.len() {
                // count width in characters (or bytes) from pos
                let mut cnt = 0usize;
                let mut end = pos;
                while end < raw.len() && cnt < width {
                    let ch = raw[end];
                    let w = if ch < 0x80 {
                        1
                    } else {
                        let mut n = 1;
                        while n < 4 && end + n < raw.len() && (raw[end + n] & 0xC0) == 0x80 {
                            n += 1;
                        }
                        n
                    };
                    if by_chars && w > 1 {
                        cnt += 1;
                        end += w;
                    } else {
                        cnt += 1;
                        end += 1;
                    }
                }
                let mut cut = end;
                if spaces {
                    let mut last = None;
                    for k in pos..end {
                        if raw[k] == b' ' {
                            last = Some(k);
                        }
                    }
                    if let Some(k) = last {
                        cut = k + 1;
                    }
                }
                if stdout.write_all(&raw[pos..cut]).is_err() {
                    return 1;
                }
                if cut < raw.len() {
                    if stdout.write_all(b"\n").is_err() {
                        return 1;
                    }
                }
                pos = cut;
            }
            if trailing {
                if stdout.write_all(b"\n").is_err() {
                    return 1;
                }
            }
        }
    }
    status
}
