
use crate::{In, Out};

pub fn run(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut body = "t".to_string();
    let mut delim = "\\:".to_string();
    let mut increment = 1i64;
    let mut join_blank = 1usize;
    let mut format = "rn".to_string();
    let mut no_renumber = false;
    let mut sep = "\t".to_string();
    let mut start = 1i64;
    let mut width = 6usize;

    let mut files: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--help" {
            writeln!(stderr, "Usage: nl [OPTIONS] [FILE]").unwrap();
            return 0;
        }
        if a == "--version" {
            writeln!(stderr, "nl (coreutils)").unwrap();
            return 0;
        }
        if a.starts_with("--") {
            let name = &a[2..];
            let mut taken = false;
            match name {
                "body-numbering" | "footer-numbering" | "header-numbering" | "section-delimiter"
                | "line-increment" | "join-blank-lines" | "number-format" | "number-separator"
                | "starting-line-number" | "number-width" => {
                    if i + 1 >= args.len() {
                        writeln!(stderr, "nl: missing argument after --{name}").unwrap();
                        return 2;
                    }
                    let v = args[i + 1].clone();
                    match name {
                        "body-numbering" => body = v,
                        "footer-numbering" | "header-numbering" => {}
                        "section-delimiter" => delim = v,
                        "line-increment" => increment = v.parse().unwrap_or(1),
                        "join-blank-lines" => join_blank = v.parse().unwrap_or(1),
                        "number-format" => format = v,
                        "number-separator" => sep = v,
                        "starting-line-number" => start = v.parse().unwrap_or(1),
                        "number-width" => width = v.parse().unwrap_or(6),
                        _ => {}
                    }
                    taken = true;
                }
                "no-renumber" => no_renumber = true,
                _ => {
                    writeln!(stderr, "nl: unknown option --{name}").unwrap();
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
                    'p' => no_renumber = true,
                    'b' | 'f' | 'h' | 'd' | 'i' | 'l' | 'n' | 's' | 'v' | 'w' => {
                        if i + 1 >= args.len() {
                            writeln!(stderr, "nl: missing argument after -{c}").unwrap();
                            return 2;
                        }
                        let v = args[i + 1].clone();
                        match c {
                            'b' => body = v,
                            'd' => delim = v,
                            'i' => increment = v.parse().unwrap_or(1),
                            'l' => join_blank = v.parse().unwrap_or(1),
                            'n' => format = v,
                            's' => sep = v,
                            'v' => start = v.parse().unwrap_or(1),
                            'w' => width = v.parse().unwrap_or(6),
                            _ => {}
                        }
                        taken = true;
                    }
                    _ => {
                        writeln!(stderr, "nl: unknown option -{c}").unwrap();
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

    if !matches!(format.as_str(), "ln" | "rn" | "rz") {
        writeln!(stderr, "nl: invalid number format: {format}").unwrap();
        return 2;
    }
    if !matches!(body.as_str(), "a" | "t" | "n") && !body.starts_with('p') {
        writeln!(stderr, "nl: invalid numbering style: {body}").unwrap();
        return 2;
    }

    let sources: Vec<String> = if files.is_empty() {
        vec!["-".to_string()]
    } else {
        files
    };
    let mut status = 0;
    let mut num = start;
    let mut blanks = 0usize;

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
                    writeln!(stderr, "nl: {f}: {e}").unwrap();
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
            let line = String::from_utf8_lossy(raw).into_owned();
            if line == delim {
                if stdout.write_all(line.as_bytes()).is_err() { return 1; }
                if stdout.write_all(b"\n").is_err() { return 1; }
                if !no_renumber {
                    num = start;
                }
                blanks = 0;
                continue;
            }
            let numbered = body == "a"
                || (body == "t" && !line.is_empty())
                || (body.starts_with('p') && line.contains(&body[1..]));
            if !numbered {
                if stdout.write_all(line.as_bytes()).is_err() { return 1; }
                if stdout.write_all(b"\n").is_err() { return 1; }
                continue;
            }
            if line.is_empty() {
                blanks += 1;
                if blanks < join_blank {
                    // inside an incomplete blank group: padding only, no number
                    if stdout.write_all(" ".repeat(width).as_bytes()).is_err() { return 1; }
                    if stdout.write_all(sep.as_bytes()).is_err() { return 1; }
                    if stdout.write_all(b"\n").is_err() { return 1; }
                    continue;
                }
                blanks = 0;
            } else if blanks > 0 && blanks < join_blank {
                // the group ends here; this line carries the group number
                blanks = 0;
            }
            if body == "n" {
                if stdout.write_all(line.as_bytes()).is_err() { return 1; }
                if stdout.write_all(b"\n").is_err() { return 1; }
                continue;
            }
            let n = num.to_string();
            let shown = if n.len() >= width {
                n
            } else {
                match format.as_str() {
                    "ln" => format!("{n:<width$}"),
                    "rz" => format!("{n:0>width$}"),
                    _ => format!("{n:>width$}"),
                }
            };
            if stdout.write_all(shown.as_bytes()).is_err() { return 1; }
            if stdout.write_all(sep.as_bytes()).is_err() { return 1; }
            if stdout.write_all(line.as_bytes()).is_err() { return 1; }
            if stdout.write_all(b"\n").is_err() { return 1; }
            num += increment;
        }
    }
    status
}
