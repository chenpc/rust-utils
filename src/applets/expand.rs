
use crate::In;
use crate::Out;

pub fn tab_stops(spec: &str) -> Vec<usize> {
    if spec.contains(',') {
        let mut v: Vec<usize> = spec
            .split(',')
            .filter_map(|p| p.parse::<usize>().ok())
            .filter(|&n| n > 0)
            .collect();
        v.sort_unstable();
        v
    } else {
        let n = spec.parse::<usize>().ok().filter(|&n| n > 0).unwrap_or(8);
        vec![n]
    }
}

pub fn next_stop(col: usize, stops: &[usize]) -> usize {
    if stops.len() == 1 {
        let n = stops[0];
        ((col / n) + 1) * n
    } else {
        for &s in stops {
            if s > col {
                return s;
            }
        }
        col + stops[stops.len() - 1]
    }
}

pub fn char_width(b: u8) -> usize {
    if b < 0x80 {
        return 1;
    }
    // Number of leading 1 bits in the UTF-8 lead byte = encoded length.
    let mut w = 0;
    while w < 4 && (b >> (7 - w)) & 1 == 1 {
        w += 1;
    }
    w.max(1)
}

/// expand: convert tabs to spaces.
pub fn run(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut initial = false;
    let mut stops = vec![8usize];
    let mut utf8 = true;
    let mut files: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--help" || a == "-h" {
            writeln!(stderr, "Usage: expand [OPTIONS] [FILE]").unwrap();
            return 0;
        }
        if a == "--version" || a == "-V" {
            writeln!(stderr, "expand (coreutils)").unwrap();
            return 0;
        }
        if a == "-i" || a == "--initial" {
            initial = true;
            i += 1;
            continue;
        }
        if a == "-U" || a == "--no-utf8" {
            utf8 = false;
            i += 1;
            continue;
        }
        if a == "-t" || a == "--tabs" {
            if i + 1 >= args.len() {
                writeln!(stderr, "expand: missing argument after {a}").unwrap();
                return 2;
            }
            stops = tab_stops(&args[i + 1]);
            i += 2;
            continue;
        }
        if a.starts_with('-') && a != "-" {
            writeln!(stderr, "expand: unknown option {a}").unwrap();
            return 2;
        }
        files.push(a.clone());
        i += 1;
    }

    if stops.is_empty() {
        stops = vec![8];
    }

    let mut status = 0;
    let sources: Vec<String> = if files.is_empty() {
        vec!["-".to_string()]
    } else {
        files
    };
    for f in sources {
        let data = if f == "-" {
            let mut d = Vec::new();
            if stdin.read_to_end(&mut d).is_err() {
                return 1;
            }
            d
        } else {
            match std::fs::read(&f) {
                Ok(d) => d,
                Err(e) => {
                    writeln!(stderr, "expand: {f}: {e}").unwrap();
                    status = 1;
                    continue;
                }
            }
        };
        let mut pos = 0;
        while pos < data.len() {
            let end = match data[pos..].iter().position(|&c| c == b'\n') {
                Some(p) => pos + p,
                None => data.len(),
            };
            let raw = &data[pos..end];
            let mut col = 0usize;
            let mut leading = true;
            let mut j = 0;
            while j < raw.len() {
                let b = raw[j];
                if b == b'\t' {
                    if initial && !leading {
                        if stdout.write_all(b"\t").is_err() {
                            return 1;
                        }
                        col += 1;
                    } else {
                        let target = next_stop(col, &stops);
                        let sp = " ".repeat(target - col);
                        if stdout.write_all(sp.as_bytes()).is_err() {
                            return 1;
                        }
                        col = target;
                    }
                    j += 1;
                    continue;
                }
                if b != b' ' {
                    leading = false;
                }
                let w = char_width(b);
                if stdout.write_all(&raw[j..j + w]).is_err() {
                    return 1;
                }
                col += if utf8 { 1 } else { w };
                j += w;
            }
            if end < data.len() {
                if stdout.write_all(b"\n").is_err() {
                    return 1;
                }
            }
            pos = end + 1;
        }
    }
    status
}
