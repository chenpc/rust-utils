
use crate::{In, Out};

fn key(
    l: &[u8],
    skip_chars: usize,
    skip_fields: usize,
    check: Option<usize>,
    ic: bool,
) -> Vec<u8> {
    let mut sf = skip_fields;
    let mut pos = 0;
    while sf > 0 && pos < l.len() {
        while pos < l.len() && l[pos].is_ascii_whitespace() {
            pos += 1;
        }
        while pos < l.len() && !l[pos].is_ascii_whitespace() {
            pos += 1;
        }
        sf -= 1;
    }
    pos += skip_chars;
    let end = match check {
        Some(n) => l.len().min(pos + n),
        None => l.len(),
    };
    let k = l.get(pos..end).unwrap_or(&[]).to_vec();
    if ic {
        k.iter().map(|b| b.to_ascii_lowercase()).collect()
    } else {
        k
    }
}

pub fn run(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut all_repeated = false;
    let mut group = false;
    let mut group_method = "separate".to_string();
    let mut delim_method = "none".to_string();
    let mut check_chars: Option<usize> = None;
    let mut count = false;
    let mut ignore_case = false;
    let mut repeated_only = false;
    let mut skip_chars = 0usize;
    let mut skip_fields = 0usize;
    let mut unique_only = false;
    let mut nul = false;

    let mut files: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--help" {
            writeln!(stderr, "Usage: uniq [OPTIONS] [FILE]").unwrap();
            return 0;
        }
        if a == "--version" {
            writeln!(stderr, "uniq (coreutils)").unwrap();
            return 0;
        }
        if a.starts_with("--") {
            let (name, val) = match a[2..].find('=') {
                Some(p) => (a[2..2 + p].to_string(), Some(a[2 + p + 1..].to_string())),
                None => (a[2..].to_string(), None),
            };
            match name.as_str() {
                "all-repeated" => {
                    all_repeated = true;
                    if let Some(v) = val {
                        delim_method = v;
                    }
                }
                "group" => {
                    group = true;
                    if let Some(v) = val {
                        group_method = v;
                    }
                }
                "check-chars" => check_chars = Some(usize::MAX),
                "count" => count = true,
                "ignore-case" => ignore_case = true,
                "repeated" => repeated_only = true,
                "skip-chars" => skip_chars = usize::MAX,
                "skip-fields" => skip_fields = usize::MAX,
                "unique" => unique_only = true,
                "zero-terminated" => nul = true,
                _ => {
                    writeln!(stderr, "uniq: unknown option --{name}").unwrap();
                    return 2;
                }
            }
            i += 1;
            continue;
        }
        if a.starts_with('-') && a.len() > 1 {
            let mut taken = false;
            for c in a[1..].chars() {
                match c {
                    'D' => all_repeated = true,
                    'w' | 's' | 'f' => {
                        if i + 1 >= args.len() {
                            writeln!(stderr, "uniq: missing argument after -{c}").unwrap();
                            return 2;
                        }
                        let n: usize = args[i + 1].parse().unwrap_or(usize::MAX);
                        match c {
                            'w' => check_chars = Some(n),
                            's' => skip_chars = n,
                            _ => skip_fields = n,
                        }
                        taken = true;
                    }
                    'c' => count = true,
                    'i' => ignore_case = true,
                    'd' => repeated_only = true,
                    'u' => unique_only = true,
                    'z' => nul = true,
                    _ => {
                        writeln!(stderr, "uniq: unknown option -{c}").unwrap();
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

    let term = if nul { 0u8 } else { b'\n' };
    let sources: Vec<String> = if files.is_empty() {
        vec!["-".to_string()]
    } else {
        files
    };
    let mut status = 0;
    let mut lines: Vec<Vec<u8>> = Vec::new();
    for f in &sources {
        let data = if f == "-" {
            let mut b = Vec::new();
            if stdin.read_to_end(&mut b).is_err() {
                status = 1;
                continue;
            }
            b
        } else {
            match std::fs::read(f) {
                Ok(b) => b,
                Err(e) => {
                    writeln!(stderr, "uniq: {f}: {e}").unwrap();
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
            lines.push(raw.to_vec());
        }
    }

    let mut groups: Vec<(Vec<u8>, Vec<usize>)> = Vec::new();
    for (idx, l) in lines.iter().enumerate() {
        let k = key(l, skip_chars, skip_fields, check_chars, ignore_case);
        if groups.last().map(|g| g.0 == k).unwrap_or(false) {
            groups.last_mut().unwrap().1.push(idx);
        } else {
            groups.push((k, vec![idx]));
        }
    }

    for (gi, (_k, members)) in groups.iter().enumerate() {
        if group {
            if gi > 0 && (group_method == "separate" || group_method == "both" || group_method == "prepend") {
                if stdout.write_all(&[term]).is_err() {
                    return 1;
                }
            }
            for (j, &mi) in members.iter().enumerate() {
                if j > 0 && group_method == "prepend" {
                    continue;
                }
                if stdout.write_all(&lines[mi]).is_err() {
                    return 1;
                }
                if stdout.write_all(&[term]).is_err() {
                    return 1;
                }
            }
            if group_method == "append" || group_method == "both" {
                if stdout.write_all(&[term]).is_err() {
                    return 1;
                }
            }
            continue;
        }
        if unique_only && members.len() != 1 {
            continue;
        }
        if (repeated_only || all_repeated) && members.len() < 2 {
            continue;
        }
        // --all-repeated delimiters: prepend = before every group, separate = between groups.
        if all_repeated && (delim_method == "prepend" || (delim_method == "separate" && gi > 0)) {
            if stdout.write_all(&[term]).is_err() {
                return 1;
            }
        }
        let reps = if all_repeated { members.len() } else { 1 };
        for j in 0..reps {
            if count {
                write!(stdout, "{:7} ", members.len()).unwrap();
            }
            if stdout.write_all(&lines[members[j]]).is_err() {
                return 1;
            }
            if stdout.write_all(&[term]).is_err() {
                return 1;
            }
        }
        if all_repeated && (delim_method == "append" || delim_method == "both") {
            if stdout.write_all(&[term]).is_err() {
                return 1;
            }
        }
    }
    status
}
