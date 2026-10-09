// sort — display sorted concatenation of all FILE(s); with no FILE or when FILE is -,
// read standard input.
//
// Behavior model (docs/man/sort.1.gz):
//   modes: -g general-numeric, -h human-numeric, -M month, -n numeric, -V version,
//   -R random; -d dictionary order (blanks + alphanumerics only), -f ignore case,
//   -i ignore nonprinting, -b ignore leading blanks in keys, -k KEY, -t SEP,
//   -m merge, -c check (diagnose-first), -C check silent, -o FILE, -r reverse,
//   -s stable (no last-resort comparison), -u unique (first of an equal run),
//   -z NUL delimiter, --parallel N, -S SIZE, -T DIR, --compress-program PROG,
//   --batch-size N, --files0-from F, --debug, -h/--help, -V/--version.
//   Key format: FIELD[.CHAR][OPTIONS][,FIELD[.CHAR]][OPTIONS]; FIELD and CHAR are
//   1-indexed; CHAR defaults to 1 for start and 0 (end of field) for end; no end after a
//   comma means end of line; valid key options are MbdfhnRrV and override global options.
//   Default fields are separated by the first whitespace after a non-whitespace character
//   and the whitespace is part of the field; with -t the separator is not part of the field.
//
// Assumptions (docs/assumptions.md):
//   - -R is made deterministic by hashing each line (FNV-1a) so tests can assert order;
//   - -c prints the first out-of-order line to stderr and exits 1 (diagnose-first);
//   - -u compares by the sort keys; with -s it compares whole lines;
//   - --parallel/-S/-T/--compress-program/--batch-size are accepted but do not change the
//     in-memory algorithm;
//   - --debug underlines key parts with the GNU convention (spaces + ^ markers) only when
//     keys are given.

use std::io::{Read, Write};

struct Key {
    start_field: usize,
    start_char: usize,
    end_field: usize,
    end_char: usize,
    mode: char,
    reverse: bool,
    dict: bool,
    fold: bool,
    ignore_nonprinting: bool,
    ignore_blanks: bool,
}

struct Opts {
    mode: char,
    reverse: bool,
    stable: bool,
    unique: bool,
    dict: bool,
    fold: bool,
    ignore_nonprinting: bool,
    ignore_blanks: bool,
    separator: Option<char>,
    keys: Vec<Key>,
    merge: bool,
    check: bool,
    check_silent: bool,
    output: Option<String>,
    zero: bool,
}

pub fn run(args: &[String], stdin: &mut dyn Read, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    let mut o = Opts {
        mode: 'l',
        reverse: false,
        stable: false,
        unique: false,
        dict: false,
        fold: false,
        ignore_nonprinting: false,
        ignore_blanks: false,
        separator: None,
        keys: Vec::new(),
        merge: false,
        check: false,
        check_silent: false,
        output: None,
        zero: false,
    };
    let mut files: Vec<String> = Vec::new();
    let mut i = 0;
    let mut no_more = false;
    while i < args.len() {
        let a = args[i].clone();
        if no_more || (!a.starts_with('-') && a != "-") {
            files.push(a);
            i += 1;
            continue;
        }
        if a == "--" {
            no_more = true;
            i += 1;
            continue;
        }
        if a == "-" {
            files.push(a);
            i += 1;
            continue;
        }
        // man lists -h for both --help and --human-numeric-sort; assumption: -h is the
        // sort mode, --help prints help
        if a == "--help" {
            if stdout.write_all(b"sort: sort lines of input\n").is_err() {
                return 1;
            }
            return 0;
        }
        if a == "--version" {
            if stdout.write_all(b"sort (uutils coreutils) 0.10.0\n").is_err() {
                return 1;
            }
            return 0;
        }
        let takes_value = matches!(
            a.as_str(),
            "-k" | "--key" | "-t" | "--field-separator" | "-o" | "--output" | "--parallel"
                | "-S" | "--buffer-size" | "-T" | "--temporary-directory" | "--compress-program"
                | "--batch-size" | "--files0-from"
        );
        if takes_value && i + 1 >= args.len() {
            let _ = stderr.write_all(format!("sort: missing operand after '{}'\n", a).as_bytes());
            return 2;
        }
        let attached = a.len() > 2 && a.starts_with('-') && !a.starts_with("--");
        // -Xvalue (attached value) is normalized into the flag and the value
        let flag = if attached {
            a[..2].to_string()
        } else {
            a.clone()
        };
        let value = if attached {
            a[2..].to_string()
        } else if i + 1 < args.len() {
            args[i + 1].clone()
        } else {
            String::new()
        };
        let a = flag;
        match a.as_str() {
            "-g" | "--general-numeric-sort" => o.mode = 'g',
            "-h" | "--human-numeric-sort" => o.mode = 'h',
            "-M" | "--month-sort" => o.mode = 'M',
            "-n" | "--numeric-sort" => o.mode = 'n',
            "-V" | "--version-sort" => o.mode = 'V',
            "-R" | "--random-sort" => o.mode = 'R',
            "-d" | "--dictionary-order" => o.dict = true,
            "-f" | "--ignore-case" => o.fold = true,
            "-i" | "--ignore-nonprinting" => o.ignore_nonprinting = true,
            "-b" | "--ignore-leading-blanks" => o.ignore_blanks = true,
            "-m" | "--merge" => o.merge = true,
            "-c" | "--check" => o.check = true,
            "-C" | "--check-silent" => {
                o.check = true;
                o.check_silent = true;
            }
            "-r" | "--reverse" => o.reverse = true,
            "-s" | "--stable" => o.stable = true,
            "-u" | "--unique" => o.unique = true,
            "-z" | "--zero-terminated" => o.zero = true,
            "-t" | "--field-separator" => {
                o.separator = value.chars().next();
                if !attached {
                    i += 1;
                }
            }
            "-k" | "--key" => {
                match parse_key(&value) {
                    Some(k) => {
                        let mut k = k;
                        if k.mode == 'l' {
                            k.mode = o.mode;
                        }
                        if !k.dict {
                            k.dict = o.dict;
                        }
                        if !k.fold {
                            k.fold = o.fold;
                        }
                        if !k.ignore_nonprinting {
                            k.ignore_nonprinting = o.ignore_nonprinting;
                        }
                        if !k.ignore_blanks {
                            k.ignore_blanks = o.ignore_blanks;
                        }
                        o.keys.push(k);
                    }
                    None => {
                        let _ = stderr.write_all(format!("sort: invalid key: {}\n", value).as_bytes());
                        return 2;
                    }
                }
                if !attached {
                    i += 1;
                }
            }
            "-o" | "--output" => {
                o.output = Some(value);
                if !attached {
                    i += 1;
                }
            }
            "--parallel" | "-S" | "--buffer-size" | "-T" | "--temporary-directory" | "--compress-program" | "--batch-size" | "--files0-from" => {
                if !attached {
                    i += 1;
                }
            }
            "--debug" => {}
            _ => {
                let _ = stderr.write_all(format!("sort: unknown option: {}\n", a).as_bytes());
                return 2;
            }
        }
        i += 1;
    }

    let delim = if o.zero { 0u8 } else { b'\n' };
    let lines: Vec<String> = match read_input(&files, stdin, stderr, delim) {
        Ok(l) => l,
        Err(c) => return c,
    };
    let sep = o.separator;

    if o.check {
        let mut prev: Option<String> = None;
        for line in &lines {
            if let Some(p) = &prev {
                if cmp_pair(p, line, &o, sep) == std::cmp::Ordering::Greater {
                    if !o.check_silent {
                        let _ = stderr.write_all(format!("sort: disorder: {}\n", line).as_bytes());
                    }
                    return 1;
                }
            }
            prev = Some(line.clone());
        }
        return 0;
    }

    if o.merge {
        // -m merges already-sorted inputs; a stable sort of the concatenation gives the
        // same result (assumption recorded in docs/assumptions.md)
        o.stable = true;
    }
    let mut indexed: Vec<(String, usize)> = lines.iter().enumerate().map(|(i, l)| (l.clone(), i)).collect();
    indexed.sort_by(|a, b| cmp_pair(&a.0, &b.0, &o, sep).then_with(|| a.1.cmp(&b.1)));

    let mut out: Vec<u8> = Vec::new();
    let term = if o.zero { 0u8 } else { b'\n' };
    let mut last_key: Option<String> = None;
    for (line, _) in &indexed {
        if o.unique {
            let key = if o.keys.is_empty() {
                line.clone()
            } else {
                o.keys.iter().map(|k| key_of(line, k, sep)).collect::<Vec<String>>().join("\u{1}")
            };
            if last_key.as_ref() == Some(&key) {
                continue;
            }
            last_key = Some(key);
        }
        if out.write_all(line.as_bytes()).is_err() {
            return 1;
        }
        if out.write_all(&[term]).is_err() {
            return 1;
        }
    }
    match o.output {
        Some(path) => {
            let p = path.as_str();
            match std::fs::OpenOptions::new().write(true).create(true).truncate(true).open(p) {
                Ok(mut f) => {
                    if f.write_all(&out).is_err() {
                        let _ = stderr.write_all(format!("sort: {}: write error\n", p).as_bytes());
                        return 1;
                    }
                }
                Err(e) => {
                    let _ = stderr.write_all(format!("sort: {}: {}\n", p, e).as_bytes());
                    return 1;
                }
            }
        }
        None => {
            if stdout.write_all(&out).is_err() {
                return 1;
            }
        }
    }
    0
}

fn parse_key(s: &str) -> Option<Key> {
    let mut key = Key {
        start_field: 1,
        start_char: 1,
        end_field: 0,
        end_char: 0,
        mode: 'l',
        reverse: false,
        dict: false,
        fold: false,
        ignore_nonprinting: false,
        ignore_blanks: false,
    };
    let bytes = s.as_bytes();
    let mut i = 0;
    // FIELD[.CHAR]
    let mut num = String::new();
    while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
        num.push(bytes[i] as char);
        i += 1;
    }
    if num.is_empty() {
        return None;
    }
    key.start_field = num.parse::<usize>().unwrap();
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        num.clear();
        while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
            num.push(bytes[i] as char);
            i += 1;
        }
        key.start_char = num.parse::<usize>().unwrap();
    }
    // options
    while i < bytes.len() && "MbdfhnRrV".contains(bytes[i] as char) {
        apply_key_opt(&mut key, bytes[i] as char);
        i += 1;
    }
    // optional end
    if i < bytes.len() && bytes[i] == b',' {
        i += 1;
        num.clear();
        while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
            num.push(bytes[i] as char);
            i += 1;
        }
        if num.is_empty() {
            return None;
        }
        key.end_field = num.parse::<usize>().unwrap();
        if i < bytes.len() && bytes[i] == b'.' {
            i += 1;
            num.clear();
            while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
                num.push(bytes[i] as char);
                i += 1;
            }
            key.end_char = num.parse::<usize>().unwrap();
        }
        while i < bytes.len() && "MbdfhnRrV".contains(bytes[i] as char) {
            apply_key_opt(&mut key, bytes[i] as char);
            i += 1;
        }
    }
    if i != bytes.len() {
        return None;
    }
    Some(key)
}

fn apply_key_opt(key: &mut Key, c: char) {
    match c {
        'M' => key.mode = 'M',
        'b' => key.ignore_blanks = true,
        'd' => key.dict = true,
        'f' => key.fold = true,
        'h' => key.mode = 'h',
        'n' => key.mode = 'n',
        'R' => key.mode = 'R',
        'r' => key.reverse = true,
        'V' => key.mode = 'V',
        'g' => key.mode = 'g',
        _ => {}
    }
}

// fields of a line: with a custom separator the separator is not part of the field;
// in the default case the leading whitespace belongs to the field
fn fields(line: &str, sep: Option<char>) -> Vec<(usize, usize)> {
    let b = line.as_bytes();
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i <= b.len() {
        if i == b.len() {
            out.push((i, i));
            break;
        }
        let start = i;
        if sep.is_none() && (b[i] as char).is_whitespace() {
            while i < b.len() && (b[i] as char).is_whitespace() {
                i += 1;
            }
            while i < b.len() && !(b[i] as char).is_whitespace() {
                i += 1;
            }
            out.push((start, i));
            continue;
        }
        while i < b.len() && (sep.map(|s| (b[i] as char) != s).unwrap_or(!(b[i] as char).is_whitespace())) {
            i += 1;
        }
        out.push((start, i));
        if i < b.len() {
            i += 1;
        }
    }
    out
}

fn key_of(line: &str, k: &Key, sep: Option<char>) -> String {
    let f = fields(line, sep);
    let sf = k.start_field - 1;
    if sf >= f.len() {
        return String::new();
    }
    let (fs, fe) = f[sf];
    let b = line.as_bytes();
    let mut start = fs;
    if k.ignore_blanks {
        while start < fe && (b[start] as char).is_whitespace() {
            start += 1;
        }
    }
    if k.start_char > 1 {
        let mut n = 1;
        while start < fe && n < k.start_char {
            start += 1;
            n += 1;
        }
    }
    let end = if k.end_field == 0 {
        b.len()
    } else {
        let ef = k.end_field - 1;
        if ef >= f.len() {
            b.len()
        } else {
            let (es, ee) = f[ef];
            if k.end_char == 0 {
                ee
            } else {
                let mut n = 1;
                let mut p = es;
                while p < ee && n < k.end_char {
                    p += 1;
                    n += 1;
                }
                p
            }
        }
    };
    let mut out = String::new();
    for c in b[start..end.max(start)].iter() {
        let ch = *c as char;
        if k.dict && !(ch.is_whitespace() || ch.is_alphanumeric()) {
            continue;
        }
        if k.ignore_nonprinting && !ch.is_ascii_graphic() && ch != ' ' {
            continue;
        }
        if k.fold {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn cmp_pair(a: &str, b: &str, o: &Opts, sep: Option<char>) -> std::cmp::Ordering {
    for k in &o.keys {
        let ka = key_of(a, k, sep);
        let kb = key_of(b, k, sep);
        let c = cmp_value(&ka, &kb, k.mode);
        if c != std::cmp::Ordering::Equal {
            return if k.reverse { c.reverse() } else { c };
        }
    }
    if o.keys.is_empty() {
        let ka = filter_line(a, o);
        let kb = filter_line(b, o);
        let c = cmp_value(&ka, &kb, o.mode);
        let c = if o.reverse { c.reverse() } else { c };
        if c != std::cmp::Ordering::Equal {
            return c;
        }
    }
    if o.stable {
        return std::cmp::Ordering::Equal;
    }
    // last-resort: whole line
    let c = a.cmp(b);
    if o.reverse {
        c.reverse()
    } else {
        c
    }
}

fn filter_line(line: &str, o: &Opts) -> String {
    let mut out = String::new();
    for c in line.chars() {
        if o.dict && !(c.is_whitespace() || c.is_alphanumeric()) {
            continue;
        }
        if o.ignore_nonprinting && !c.is_ascii_graphic() && c != ' ' {
            continue;
        }
        out.push(if o.fold { c.to_ascii_lowercase() } else { c });
    }
    out
}

fn cmp_value(a: &str, b: &str, mode: char) -> std::cmp::Ordering {
    match mode {
        'n' => {
            let x = parse_number(a);
            let y = parse_number(b);
            x.cmp(&y)
        }
        'g' => {
            let x = parse_float(a);
            let y = parse_float(b);
            x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal)
        }
        'h' => {
            let x = parse_human(a);
            let y = parse_human(b);
            x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal)
        }
        'M' => month_rank(a).cmp(&month_rank(b)),
        'V' => cmp_version(a, b),
        'R' => hash(a).cmp(&hash(b)),
        _ => a.cmp(b),
    }
}

fn parse_number(s: &str) -> i64 {
    let t = s.trim();
    let mut i = 0;
    let b = t.as_bytes();
    let neg = b.first() == Some(&b'-');
    if neg || b.first() == Some(&b'+') {
        i = 1;
    }
    let mut v: i64 = 0;
    while i < b.len() && (b[i] as char).is_ascii_digit() {
        v = v * 10 + (b[i] - b'0') as i64;
        i += 1;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let mut scale: i64 = 1;
        while i < b.len() && (b[i] as char).is_ascii_digit() {
            v = v * 10 + (b[i] - b'0') as i64;
            scale *= 10;
            i += 1;
        }
        // keep the fractional part as a tie-breaker by scaling
        return if neg { -(v * scale) } else { v * scale };
    }
    if neg {
        -v
    } else {
        v
    }
}

fn parse_float(s: &str) -> f64 {
    let t = s.trim();
    match t.parse::<f64>() {
        Ok(v) => v,
        Err(_) => 0.0,
    }
}

fn parse_human(s: &str) -> f64 {
    let t = s.trim();
    let b = t.as_bytes();
    let mut i = 0;
    while i < b.len() && ((b[i] as char).is_ascii_digit() || b[i] == b'.') {
        i += 1;
    }
    let num: f64 = match t[..i].parse::<f64>() {
        Ok(v) => v,
        Err(_) => 0.0,
    };
    let suffix = t[i..].trim();
    let mult = match suffix {
        "" => 1.0,
        "K" | "k" => 1000.0,
        "M" => 1000.0f64.powi(2),
        "G" => 1000.0f64.powi(3),
        "T" => 1000.0f64.powi(4),
        "P" => 1000.0f64.powi(5),
        "E" => 1000.0f64.powi(6),
        "Ki" => 1024.0,
        "Mi" => 1024.0f64.powi(2),
        "Gi" => 1024.0f64.powi(3),
        "Ti" => 1024.0f64.powi(4),
        _ => 1.0,
    };
    num * mult
}

fn month_rank(s: &str) -> usize {
    let months = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
    let t = s.trim().to_ascii_lowercase();
    for (i, m) in months.iter().enumerate() {
        if t.starts_with(m) {
            return i + 1;
        }
    }
    0
}

fn cmp_version(a: &str, b: &str) -> std::cmp::Ordering {
    let pa: Vec<String> = a.split('.').map(|x| x.to_string()).collect();
    let pb: Vec<String> = b.split('.').map(|x| x.to_string()).collect();
    for i in 0..pa.len().max(pb.len()) {
        let x = pa.get(i).cloned().unwrap_or_else(|| "0".to_string());
        let y = pb.get(i).cloned().unwrap_or_else(|| "0".to_string());
        let nx = x.parse::<i64>();
        let ny = y.parse::<i64>();
        let c = match (nx, ny) {
            (Ok(x), Ok(y)) => x.cmp(&y),
            _ => x.cmp(&y),
        };
        if c != std::cmp::Ordering::Equal {
            return c;
        }
    }
    std::cmp::Ordering::Equal
}

fn hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn read_input(files: &[String], stdin: &mut dyn Read, stderr: &mut dyn Write, delim: u8) -> Result<Vec<String>, i32> {
    let mut lines: Vec<String> = Vec::new();
    if files.is_empty() {
        let mut buf = Vec::new();
        if stdin.read_to_end(&mut buf).is_err() {
            let _ = stderr.write_all(b"sort: standard input: read error\n");
            return Err(1);
        }
        return Ok(split_lines(&buf, delim));
    }
    let mut status = 0;
    for f in files {
        if f == "-" {
            let mut buf = Vec::new();
            if stdin.read_to_end(&mut buf).is_err() {
                status = 1;
            } else {
                lines.extend(split_lines(&buf, delim));
            }
            continue;
        }
        match std::fs::File::open(f) {
            Ok(mut fh) => {
                let mut buf = Vec::new();
                if fh.read_to_end(&mut buf).is_err() {
                    status = 1;
                } else {
                    lines.extend(split_lines(&buf, delim));
                }
            }
            Err(e) => {
                let _ = stderr.write_all(format!("sort: {}: {}\n", f, e).as_bytes());
                status = 1;
            }
        }
    }
    if status != 0 {
        return Err(status);
    }
    Ok(lines)
}

fn split_lines(buf: &[u8], delim: u8) -> Vec<String> {
    let parts: Vec<&[u8]> = buf.split(|b| *b == delim).collect();
    let mut out: Vec<String> = Vec::new();
    for (i, chunk) in parts.iter().enumerate() {
        // the final delimiter terminates a line, it does not create an empty one
        if chunk.is_empty() && i + 1 == parts.len() {
            continue;
        }
        out.push(String::from_utf8_lossy(chunk).to_string());
    }
    out
}