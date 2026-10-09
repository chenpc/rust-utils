// ptx — permuted index (KWIC) of the words in the input files.
//
// Behavior model (docs/man/ptx.1.gz):
//   - Output a permuted index, including context, of the words in the input files.
//   - With no FILE, or when FILE is -, read standard input. Default is '-F /'.
//   - -A auto references, -G traditional, -F flag-truncation STRING, -M macro-name STRING,
//     -O roff directives, -T TeX directives, -R right-side refs (not counted in -w),
//     -S sentence regexp, -W word regexp, -b break-file, -f ignore case, -g gap-size,
//     -i ignore-file, -o only-file, -r first field of each line is a reference,
//     -t typeset mode (default width 72 -> 100), -w width, -h, -V.
//
// Assumptions (not specified by the man page, recorded in docs/assumptions.md):
//   - a sentence is one input line (the default -S regexp matches at the start of a line);
//   - a word is a maximal run of non-break characters; the default break set is whitespace
//     and -b FILE replaces it with the characters found in FILE; -W REGEXP selects the
//     keyword by regex instead;
//   - default gap size is 3 columns, default width 72 (100 with -t);
//   - the reference is the first field of the line with -r, and the 1-based line number with -A;
//   - output line = reference + gap + keyword + gap + context, where context is the other
//     words of the line in original order; the context is truncated to fit the width and each
//     truncated side is marked with the flag string;
//   - keywords are sorted (case-folded with -f), ties keep input order;
//   - -O prints ".<macro> reference keyword context", -T prints "\\macro{reference}{keyword}{context}".

use std::io::{Read, Write};

use crate::regex;

pub fn run(args: &[String], stdin: &mut dyn Read, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    let mut auto_ref = false;
    let mut flag = "/".to_string();
    let mut macro_name = "xx".to_string();
    let mut roff = false;
    let mut tex = false;
    let mut right_refs = false;
    let mut fold_case = false;
    let mut gap = 3usize;
    let mut width = 72usize;
    let mut references = false;
    let mut word_re = String::new();
    let mut break_chars: Vec<u8> = Vec::new();
    let mut ignore: Vec<String> = Vec::new();
    let mut only: Vec<String> = Vec::new();
    let mut files: Vec<String> = Vec::new();

    let mut i = 0;
    let mut no_more_opts = false;
    while i < args.len() {
        let a = args[i].clone();
        if no_more_opts || !a.starts_with('-') || a == "-" {
            files.push(a);
            i += 1;
            continue;
        }
        if a == "--" {
            no_more_opts = true;
            i += 1;
            continue;
        }
        if a == "-h" || a == "--help" {
            if stdout.write_all(b"ptx: permuted index of file contents\n").is_err() {
                return 1;
            }
            return 0;
        }
        if a == "-V" || a == "--version" {
            if stdout.write_all(b"ptx (uutils coreutils) 0.10.0\n").is_err() {
                return 1;
            }
            return 0;
        }
        match a.as_str() {
            "-A" | "--auto-reference" => auto_ref = true,
            "-G" | "--traditional" => {}
            "-O" => roff = true,
            "-T" => tex = true,
            "-R" | "--right-side-refs" => right_refs = true,
            "-f" | "--ignore-case" => fold_case = true,
            "-r" | "--references" => references = true,
            "-t" | "--typeset-mode" => width = 100,
            "-F" | "--flag-truncation"
            | "-M" | "--macro-name"
            | "-W" | "--word-regexp"
            | "-b" | "--break-file"
            | "-g" | "--gap-size"
            | "-i" | "--ignore-file"
            | "-o" | "--only-file"
            | "-w" | "--width" => {
                if i + 1 >= args.len() {
                    let _ = stderr.write_all(b"ptx: missing operand\n");
                    return 2;
                }
                let v = args[i + 1].clone();
                i += 1;
                match a.as_str() {
                    "-F" | "--flag-truncation" => flag = v,
                    "-M" | "--macro-name" => macro_name = v,
                    "-W" | "--word-regexp" => word_re = v,
                    "-b" | "--break-file" => match read_bytes(&v) {
                        Ok(data) => break_chars = data,
                        Err(e) => {
                            let _ = stderr.write_all(format!("ptx: {}: {}\n", v, e).as_bytes());
                            return 1;
                        }
                    },
                    "-g" | "--gap-size" => match v.parse::<usize>() {
                        Ok(n) => gap = n,
                        Err(_) => {
                            let _ = stderr.write_all(format!("ptx: invalid gap size: {}\n", v).as_bytes());
                            return 2;
                        }
                    },
                    "-w" | "--width" => match v.parse::<usize>() {
                        Ok(n) => width = n,
                        Err(_) => {
                            let _ = stderr.write_all(format!("ptx: invalid width: {}\n", v).as_bytes());
                            return 2;
                        }
                    },
                    "-i" | "--ignore-file" => match read_bytes(&v) {
                        Ok(data) => ignore = split_words(&data, &break_chars),
                        Err(e) => {
                            let _ = stderr.write_all(format!("ptx: {}: {}\n", v, e).as_bytes());
                            return 1;
                        }
                    },
                    "-o" | "--only-file" => match read_bytes(&v) {
                        Ok(data) => only = split_words(&data, &break_chars),
                        Err(e) => {
                            let _ = stderr.write_all(format!("ptx: {}: {}\n", v, e).as_bytes());
                            return 1;
                        }
                    },
                    _ => {}
                }
            }
            _ => {
                let _ = stderr.write_all(format!("ptx: unknown option: {}\n", a).as_bytes());
                return 2;
            }
        }
        i += 1;
    }

    let input = read_input(&files, stdin, stderr);
    match input {
        Ok(buf) => run_index(&buf, &break_chars, &word_re, &ignore, &only, gap, width, &flag, &macro_name, fold_case, references, auto_ref, right_refs, roff, tex, stdout),
        Err(code) => code,
    }
}

fn run_index(
    input: &[u8],
    break_chars: &[u8],
    word_re: &str,
    ignore: &[String],
    only: &[String],
    gap: usize,
    width: usize,
    flag: &str,
    macro_name: &str,
    fold_case: bool,
    references: bool,
    auto_ref: bool,
    right_refs: bool,
    roff: bool,
    tex: bool,
    stdout: &mut dyn Write,
) -> i32 {
    let re = if word_re.is_empty() { None } else { Some(regex::compile(word_re.as_bytes())) };
    let mut rows: Vec<(String, String, String, String, usize)> = Vec::new();
    let mut order = 0usize;

    for (line_no, line) in input.split(|&c| c == b'\n').enumerate() {
        if line.is_empty() {
            continue;
        }
        let words = split_words(line, break_chars);
        if words.is_empty() {
            continue;
        }
        let reference = if auto_ref {
            (line_no + 1).to_string()
        } else if references {
            words[0].clone()
        } else {
            String::new()
        };
        let context_words: Vec<String> = if references && !auto_ref { words[1..].to_vec() } else { words.clone() };
        for (idx, w) in context_words.iter().enumerate() {
            if !ignore.is_empty() && ignore.contains(w) {
                continue;
            }
            if !only.is_empty() && !only.contains(w) {
                continue;
            }
            if let Some(r) = &re {
                if regex::find(r, w.as_bytes()).is_none() {
                    continue;
                }
            }
            let key = if fold_case { w.to_uppercase() } else { w.clone() };
            let left = context_words[..idx].join(" ");
            let right = context_words[idx + 1..].join(" ");
            rows.push((key, w.clone(), reference.clone(), context_of(w, &left, &right, flag, width, gap), order));
            order += 1;
        }
    }

    rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.3.cmp(&b.3)));

    for (key, kw, reference, context, _) in &rows {
        let _ = key;
        let refcol = if reference.is_empty() { String::new() } else { format!("{}{}", reference, " ".repeat(gap)) };
        let line = if roff {
            format!(".{} {} {} {}", macro_name, reference, kw, context)
        } else if tex {
            format!("\\{}{{{}}}{{{}}}{{{}}}", macro_name, reference, kw, context)
        } else if right_refs {
            let pad = width.saturating_sub(kw.len() + context.len() + gap);
            format!("{}{}{}{}{}", kw, " ".repeat(gap), context, " ".repeat(pad), reference)
        } else {
            format!("{}{}{}{}", refcol, kw, " ".repeat(gap), context)
        };
        if stdout.write_all(line.as_bytes()).is_err() {
            return 1;
        }
        if stdout.write_all(b"\n").is_err() {
            return 1;
        }
    }
    0
}

fn context_of(kw: &str, left: &str, right: &str, flag: &str, width: usize, gap: usize) -> String {
    // -w counts keyword + context; the reference is excluded
    let available = width.saturating_sub(kw.len() + gap);
    if left.is_empty() && right.is_empty() {
        return String::new();
    }
    let sep = if left.is_empty() || right.is_empty() { "" } else { " " };
    if left.len() + sep.len() + right.len() <= available {
        if left.is_empty() {
            return right.to_string();
        }
        if right.is_empty() {
            return left.to_string();
        }
        return format!("{} {}{}", left, flag, right);
    }
    // truncate the right side first, then the left
    let mut l = left.to_string();
    let mut r = right.to_string();
    let mut lf = false;
    let mut rf = false;
    if l.len() + flag.len() + r.len() > available {
        let room = available.saturating_sub(l.len() + flag.len());
        if r.len() > room {
            r = r[..room].to_string();
            rf = true;
        }
    }
    if l.len() + flag.len() + r.len() > available {
        let room = available.saturating_sub(r.len() + flag.len());
        if l.len() > room {
            l = l[..room].to_string();
            lf = true;
        }
    }
    let mut out = String::new();
    if lf {
        out.push_str(flag);
    }
    out.push_str(&l);
    if !l.is_empty() && !r.is_empty() {
        out.push_str(flag);
    }
    out.push_str(&r);
    if rf {
        out.push_str(flag);
    }
    out
}

fn split_words(data: &[u8], break_chars: &[u8]) -> Vec<String> {
    let default = [b' ', b'\t', b'\n', b'\r'];
    let mut out = Vec::new();
    let mut cur = Vec::new();
    for &c in data {
        if break_chars.is_empty() {
            if default.contains(&c) {
                if !cur.is_empty() {
                    out.push(String::from_utf8_lossy(&cur).to_string());
                    cur.clear();
                }
            } else {
                cur.push(c);
            }
        } else if break_chars.contains(&c) {
            if !cur.is_empty() {
                out.push(String::from_utf8_lossy(&cur).to_string());
                cur.clear();
            }
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        out.push(String::from_utf8_lossy(&cur).to_string());
    }
    out
}

fn read_input(files: &[String], stdin: &mut dyn Read, stderr: &mut dyn Write) -> Result<Vec<u8>, i32> {
    if files.is_empty() {
        let mut buf = Vec::new();
        if stdin.read_to_end(&mut buf).is_err() {
            let _ = stderr.write_all(b"ptx: standard input: read error\n");
            return Err(1);
        }
        return Ok(buf);
    }
    let mut buf = Vec::new();
    let mut status = 0;
    for f in files {
        if f == "-" {
            let mut b = Vec::new();
            if stdin.read_to_end(&mut b).is_err() {
                let _ = stderr.write_all(b"ptx: standard input: read error\n");
                status = 1;
            } else {
                buf.extend(b);
            }
            continue;
        }
        match std::fs::File::open(f) {
            Ok(mut fh) => {
                let mut b = Vec::new();
                if fh.read_to_end(&mut b).is_err() {
                    status = 1;
                } else {
                    buf.extend(b);
                }
            }
            Err(e) => {
                let _ = stderr.write_all(format!("ptx: {}: {}\n", f, e).as_bytes());
                status = 1;
            }
        }
    }
    if status != 0 {
        Err(status)
    } else {
        Ok(buf)
    }
}

fn read_bytes(path: &str) -> std::io::Result<Vec<u8>> {
    let mut f = std::fs::File::open(path)?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    Ok(buf)
}