use crate::{In, Out};

const MAXPOS: usize = usize::MAX;

fn parse_list(s: &str, what: &str) -> Result<Vec<(usize, usize)>, String> {
    let mut out = Vec::new();
    for item in s.split(',') {
        if item.is_empty() {
            return Err(format!("invalid {what} list: empty element"));
        }
        if let Some(p) = item.find('-') {
            let a = &item[..p];
            let b = &item[p + 1..];
            let lo = if a.is_empty() {
                1
            } else {
                a.parse::<usize>().map_err(|_| format!("invalid {what} range: {item}"))?
            };
            let hi = if b.is_empty() {
                MAXPOS
            } else {
                b.parse::<usize>().map_err(|_| format!("invalid {what} range: {item}"))?
            };
            if lo > hi {
                return Err(format!("invalid range of {what} of input lines: {item}"));
            }
            out.push((lo, hi));
        } else {
            let n = item
                .parse::<usize>()
                .map_err(|_| format!("invalid {what} position: {item}"))?;
            out.push((n, n));
        }
    }
    Ok(out)
}

fn in_ranges(pos: usize, ranges: &[(usize, usize)]) -> bool {
    ranges.iter().any(|(a, b)| pos >= *a && pos <= *b)
}

pub fn run(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut mode = "";
    let mut list = String::new();
    let mut delim = b'\t';
    let mut out_delim: Vec<u8> = Vec::new();
    let mut complement = false;
    let mut only_delimited = false;
    let mut nul = false;
    let mut no_partial = false;
    let mut ws_fields = false;
    let mut files: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--help" {
            writeln!(stderr, "Usage: cut LIST [FILE]...").unwrap();
            return 0;
        }
        if a == "--version" {
            writeln!(stderr, "cut (coreutils)").unwrap();
            return 0;
        }
        if a.starts_with("--") {
            let name = &a[2..];
            let mut taken = false;
            match name {
                "bytes" => mode = "bytes",
                "characters" => mode = "chars",
                "fields" => mode = "fields",
                "complement" => complement = true,
                "only-delimited" => only_delimited = true,
                "zero-terminated" => nul = true,
                "no-partial" => no_partial = true,
                "delimiter" | "output-delimiter" => {
                    if i + 1 >= args.len() {
                        writeln!(stderr, "cut: missing argument after --{name}").unwrap();
                        return 2;
                    }
                    let v = args[i + 1].as_bytes().to_vec();
                    if name == "delimiter" {
                        delim = v[0];
                    } else {
                        out_delim = v;
                    }
                    taken = true;
                }
                _ => {
                    writeln!(stderr, "cut: unknown option --{name}").unwrap();
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
                    'b' => mode = "bytes",
                    'c' => mode = "chars",
                    'f' => mode = "fields",
                    's' => only_delimited = true,
                    'z' => nul = true,
                    'n' => no_partial = true,
                    'w' => ws_fields = true,
                    'd' | 'O' => {
                        if i + 1 >= args.len() {
                            writeln!(stderr, "cut: missing argument after -{c}").unwrap();
                            return 2;
                        }
                        let v = args[i + 1].as_bytes().to_vec();
                        if c == 'd' {
                            delim = v[0];
                        } else {
                            out_delim = v;
                        }
                        taken = true;
                    }
                    _ => {
                        writeln!(stderr, "cut: unknown option -{c}").unwrap();
                        return 2;
                    }
                }
            }
            i += if taken { 2 } else { 1 };
            continue;
        }
        if list.is_empty() {
            list = a.clone();
        } else {
            files.push(a.clone());
        }
        i += 1;
    }

    if list.is_empty() {
        writeln!(
            stderr,
            "cut: you must specify a list of bytes, characters, or fields"
        )
        .unwrap();
        return 2;
    }
    let what = match mode {
        "bytes" => "bytes",
        "chars" => "characters",
        "fields" => "fields",
        _ => {
            writeln!(
                stderr,
                "cut: you must specify a list of bytes, characters, or fields"
            )
            .unwrap();
            return 2;
        }
    };
    let ranges = match parse_list(&list, what) {
        Ok(r) => r,
        Err(e) => {
            writeln!(stderr, "cut: {e}").unwrap();
            return 2;
        }
    };
    if out_delim.is_empty() {
        out_delim = vec![delim];
    }

    let term = if nul { 0u8 } else { b'\n' };
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
                    writeln!(stderr, "cut: {f}: {e}").unwrap();
                    status = 1;
                    continue;
                }
            }
        };

        let parts: Vec<&[u8]> = data.split(|b| *b == term).collect();
        let trailing = data.last() == Some(&term);
        for (pi, raw) in parts.iter().enumerate() {
            if raw.is_empty() && trailing && pi == parts.len() - 1 {
                continue;
            }
            let mut out: Vec<u8> = Vec::new();
            if mode == "fields" {
                let fields: Vec<&[u8]> = if ws_fields {
                    let mut fs = Vec::new();
                    let mut s = 0;
                    while s < raw.len() {
                        while s < raw.len() && raw[s].is_ascii_whitespace() {
                            s += 1;
                        }
                        let mut e = s;
                        while e < raw.len() && !raw[e].is_ascii_whitespace() {
                            e += 1;
                        }
                        if e > s {
                            fs.push(&raw[s..e]);
                        }
                        s = e;
                    }
                    fs
                } else {
                    raw.split(|b| *b == delim).collect()
                };
                let had = raw.contains(&delim) || (ws_fields && raw.iter().any(|b| b.is_ascii_whitespace()));
                if only_delimited && !had {
                    continue;
                }
                for (k, fld) in fields.iter().enumerate() {
                    if in_ranges(k + 1, &ranges) == complement {
                        continue;
                    }
                    if !out.is_empty() {
                        out.extend_from_slice(&out_delim);
                    }
                    out.extend_from_slice(fld);
                }
            } else {
                let mut pos = 0;
                let mut idx = 0;
                while pos < raw.len() {
                    let ch = raw[pos];
                    let width = if ch < 0x80 {
                        1
                    } else {
                        let mut w = 1;
                        while w < 4 && pos + w < raw.len() && (raw[pos + w] & 0xC0) == 0x80 {
                            w += 1;
                        }
                        if mode == "chars" || (mode == "bytes" && no_partial) {
                            w
                        } else {
                            1
                        }
                    };
                    idx += 1;
                    if in_ranges(idx, &ranges) != complement {
                        out.extend_from_slice(&raw[pos..pos + width]);
                    }
                    pos += width;
                }
            }
            if stdout.write_all(&out).is_err() || stdout.write_all(&[term]).is_err() {
                return 1;
            }
        }
    }
    status
}
