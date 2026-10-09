

use crate::regex::{compile, is_match};
use crate::In;
use crate::Out;

enum Pat {
    Re { pat: String, skip: bool },
    Line(usize),
}

/// csplit: split a file into sections determined by context lines.
pub fn run(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut suffix = "%02d".to_string();
    let mut prefix = "xx".to_string();
    let mut keep = false;
    let mut suppress_matched = false;
    let mut digits = 2usize;
    let mut quiet = false;
    let mut elide = false;
    let mut rest: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--help" || a == "-h" {
            writeln!(stderr, "Usage: csplit [OPTIONS] FILE PATTERN").unwrap();
            return 0;
        }
        if a == "--version" || a == "-V" {
            writeln!(stderr, "csplit (coreutils)").unwrap();
            return 0;
        }
        if a == "-k" || a == "--keep-files" {
            keep = true;
            i += 1;
            continue;
        }
        if a == "-q" || a == "--quiet" {
            quiet = true;
            i += 1;
            continue;
        }
        if a == "-z" || a == "--elide-empty-files" {
            elide = true;
            i += 1;
            continue;
        }
        if a == "--suppress-matched" {
            suppress_matched = true;
            i += 1;
            continue;
        }
        if a == "-b" || a == "--suffix-format" || a == "-f" || a == "--prefix" || a == "-n" || a == "--digits" {
            if i + 1 >= args.len() {
                writeln!(stderr, "csplit: missing argument after {a}").unwrap();
                return 2;
            }
            let v = args[i + 1].clone();
            match a.as_str() {
                "-b" | "--suffix-format" => suffix = v,
                "-f" | "--prefix" => prefix = v,
                _ => digits = v.parse::<usize>().unwrap_or(2),
            }
            i += 2;
            continue;
        }
        if a.starts_with('-') && a != "-" {
            writeln!(stderr, "csplit: unknown option {a}").unwrap();
            return 2;
        }
        rest.push(a.clone());
        i += 1;
    }

    if rest.len() < 2 {
        writeln!(stderr, "csplit: missing operand").unwrap();
        return 2;
    }

    let file = rest[0].clone();
    let data = if file == "-" {
        let mut d = Vec::new();
        if stdin.read_to_end(&mut d).is_err() {
            return 1;
        }
        d
    } else {
        match std::fs::read(&file) {
            Ok(d) => d,
            Err(e) => {
                writeln!(stderr, "csplit: {file}: {e}").unwrap();
                return 1;
            }
        }
    };

    // patterns -> (pattern, repeat)
    let mut pats: Vec<(Pat, usize)> = Vec::new();
    let mut j = 1;
    while j < rest.len() {
        let raw = rest[j].clone();
        let pat = if raw.starts_with('%') && raw.len() > 1 && raw.ends_with('%') {
            Pat::Re {
                pat: raw[1..raw.len() - 1].to_string(),
                skip: true,
            }
        } else if raw.starts_with('/') && raw.len() > 1 && raw.ends_with('/') {
            Pat::Re {
                pat: raw[1..raw.len() - 1].to_string(),
                skip: false,
            }
        } else if raw.parse::<usize>().is_ok() {
            Pat::Line(raw.parse::<usize>().unwrap())
        } else {
            writeln!(stderr, "csplit: invalid pattern {raw}").unwrap();
            return 2;
        };
        j += 1;
        let mut repeat = 0;
        if j < rest.len() && (rest[j] == "{}" || rest[j].starts_with('{')) {
            let inner = rest[j].trim_start_matches('{').trim_end_matches('}');
            repeat = inner.parse::<usize>().unwrap_or(0);
            j += 1;
        }
        pats.push((pat, repeat));
    }

    // split into lines keeping the terminator
    let mut lines: Vec<Vec<u8>> = Vec::new();
    let mut start = 0;
    while start < data.len() {
        let end = match data[start..].iter().position(|&c| c == b'\n') {
            Some(p) => start + p + 1,
            None => data.len(),
        };
        lines.push(data[start..end].to_vec());
        start = end;
    }

    let mut sections: Vec<Vec<u8>> = Vec::new();
    let mut pos = 0;
    let mut searched = false;
    for (pat, repeat) in &pats {
        for _ in 0..=*repeat {
            match pat {
                Pat::Line(n) => {
                    if pos + n > lines.len() {
                        writeln!(stderr, "csplit: line {} not found", n).unwrap();
                        if keep {
                            write_sections(&sections, &prefix, &suffix, digits, elide, quiet, stdout);
                        }
                        return 1;
                    }
                    let mut sec = Vec::new();
                    for k in pos..pos + n - 1 {
                        sec.extend_from_slice(&lines[k]);
                    }
                    sections.push(sec);
                    pos = pos + n - 1;
                }
                Pat::Re { pat: p, skip } => {
                    let re = compile(p.as_bytes());
                    let mut found = None;
                    let scan = if searched { pos + 1 } else { pos };
                    for k in scan..lines.len() {
                        let line = &lines[k][..];
                        let line = if line.last() == Some(&b'\n') { &line[..line.len()-1] } else { line };
                        if is_match(&re, line) {
                            found = Some(k);
                            break;
                        }
                    }
                    let k = match found {
                        Some(k) => k,
                        None => {
                            writeln!(stderr, "csplit: {p}: line not found").unwrap();
                            if keep {
                                write_sections(&sections, &prefix, &suffix, digits, elide, quiet, stdout);
                            }
                            return 1;
                        }
                    };
                    let mut sec = Vec::new();
                    for x in pos..k {
                        sec.extend_from_slice(&lines[x]);
                    }
                    sections.push(sec);
                    searched = true;
                    if *skip || suppress_matched {
                        pos = k + 1;
                    } else {
                        pos = k;
                    }
                }
            }
        }
    }

    if pos < lines.len() {
        let mut sec = Vec::new();
        for x in pos..lines.len() {
            sec.extend_from_slice(&lines[x]);
        }
        sections.push(sec);
    }

    write_sections(&sections, &prefix, &suffix, digits, elide, quiet, stdout);
    0
}

fn format_name(prefix: &str, suffix: &str, idx: usize, digits: usize) -> String {
    if suffix != "%02d" && suffix.contains('%') {
        if let Some(pos) = suffix.find('%') {
            let head = &suffix[..pos];
            let rest = &suffix[pos + 1..];
            let mut width = 0usize;
            let mut k = 0;
            while k < rest.len() && rest.as_bytes()[k].is_ascii_digit() {
                width = width * 10 + (rest.as_bytes()[k] - b'0') as usize;
                k += 1;
            }
            if k < rest.len() && rest.as_bytes()[k] == b'd' {
                let tail = &rest[k + 1..];
                if width > 0 {
                    return format!("{prefix}{head}{idx:0>width$}{tail}");
                }
                return format!("{prefix}{head}{idx}{tail}");
            }
        }
    }
    format!("{prefix}{idx:0>digits$}")
}

fn write_sections(
    sections: &[Vec<u8>],
    prefix: &str,
    suffix: &str,
    digits: usize,
    elide: bool,
    quiet: bool,
    stdout: Out,
) {
    for (i, sec) in sections.iter().enumerate() {
        if elide && sec.is_empty() {
            continue;
        }
        let name = format_name(prefix, suffix, i, digits);
        if std::fs::write(&name, sec).is_err() {
            writeln!(stdout, "csplit: cannot write {name}").unwrap();
            continue;
        }
        if !quiet {
            writeln!(stdout, "{}", sec.len()).unwrap();
        }
    }
}
