use std::collections::BTreeMap;

use crate::In;
use crate::Out;

/// join: for each pair of input lines with identical join fields, write a line.
pub fn run(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut add1 = false;
    let mut add2 = false;
    let mut only1 = false;
    let mut only2 = false;
    let mut empty = String::new();
    let mut ignore_case = false;
    let mut f1 = 0usize;
    let mut f2 = 0usize;
    let mut out_fmt: Vec<String> = Vec::new();
    let mut delim: Option<char> = None;
    let mut check = true;
    let mut header = false;
    let mut zero = false;
    let mut files: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--help" || a == "-h" {
            writeln!(stderr, "Usage: join [OPTIONS] FILE1 FILE2").unwrap();
            return 0;
        }
        if a == "--version" || a == "-V" {
            writeln!(stderr, "join (coreutils)").unwrap();
            return 0;
        }
        if a == "--header" {
            header = true;
            i += 1;
            continue;
        }
        if a == "--check-order" {
            check = true;
            i += 1;
            continue;
        }
        if a == "--nocheck-order" {
            check = false;
            i += 1;
            continue;
        }
        if a == "-i" || a == "--ignore-case" {
            ignore_case = true;
            i += 1;
            continue;
        }
        if a == "-z" || a == "--zero-terminated" {
            zero = true;
            i += 1;
            continue;
        }
        if a == "-a" || a == "-v" {
            if i + 1 >= args.len() {
                writeln!(stderr, "join: missing argument after {a}").unwrap();
                return 2;
            }
            let n = args[i + 1].parse::<usize>().unwrap_or(0);
            if a == "-a" {
                if n == 1 {
                    add1 = true;
                } else {
                    add2 = true;
                }
            } else if n == 1 {
                only1 = true;
            } else {
                only2 = true;
            }
            i += 2;
            continue;
        }
        if a == "-e" || a == "-o" || a == "-j" || a == "-1" || a == "-2" || a == "-t" || a == "--output-delimiter" {
            if i + 1 >= args.len() {
                writeln!(stderr, "join: missing argument after {a}").unwrap();
                return 2;
            }
            let v = args[i + 1].clone();
            match a.as_str() {
                "-e" => empty = v,
                "-o" => out_fmt = v.split(|c: char| c == ',' || c.is_whitespace()).filter(|s| !s.is_empty()).map(|s| s.to_string()).collect(),
                "-j" => {
                    let n = v.parse::<usize>().unwrap_or(1) - 1;
                    f1 = n;
                    f2 = n;
                }
                "-1" => f1 = v.parse::<usize>().unwrap_or(1) - 1,
                "-2" => f2 = v.parse::<usize>().unwrap_or(1) - 1,
                _ => delim = Some(v.chars().next().unwrap_or(' ')),
            }
            i += 2;
            continue;
        }
        if a.starts_with('-') && a != "-" {
            writeln!(stderr, "join: unknown option {a}").unwrap();
            return 2;
        }
        files.push(a.clone());
        i += 1;
    }

    if files.len() != 2 {
        writeln!(stderr, "join: missing operand").unwrap();
        return 2;
    }

    let term = if zero { b'\0' } else { b'\n' };
    let mut read_all = |name: &str| -> Result<Vec<Vec<u8>>, ()> {
        let data = if name == "-" {
            let mut d = Vec::new();
            if stdin.read_to_end(&mut d).is_err() {
                return Err(());
            }
            d
        } else {
            match std::fs::read(name) {
                Ok(d) => d,
                Err(e) => {
                    writeln!(stderr, "join: {name}: {e}").unwrap();
                    return Err(());
                }
            }
        };
        let mut lines = Vec::new();
        let mut start = 0;
        while start < data.len() {
            let end = match data[start..].iter().position(|&c| c == term) {
                Some(p) => start + p,
                None => data.len(),
            };
            lines.push(data[start..end].to_vec());
            start = end + 1;
        }
        Ok(lines)
    };

    let l1 = match read_all(&files[0]) {
        Ok(l) => l,
        Err(_) => return 1,
    };
    let l2 = match read_all(&files[1]) {
        Ok(l) => l,
        Err(_) => return 1,
    };

    let split_fields = |line: &[u8]| -> Vec<String> {
        if let Some(d) = delim {
            line.split(|&c| c == d as u8)
                .map(|s| String::from_utf8_lossy(s).into_owned())
                .collect()
        } else {
            line.split(|&c| c.is_ascii_whitespace())
                .filter(|s| !s.is_empty())
                .map(|s| String::from_utf8_lossy(s).into_owned())
                .collect()
        }
    };

    let key_of = |fields: &[String], idx: usize| -> String {
        let k = fields.get(idx).cloned().unwrap_or_default();
        if ignore_case {
            k.to_lowercase()
        } else {
            k
        }
    };

    if check {
        for (idx, lines) in [&l1, &l2].iter().enumerate() {
            let mut prev: Option<String> = None;
            for line in lines.iter().skip(header as usize) {
                let k = key_of(&split_fields(line), if idx == 0 { f1 } else { f2 });
                if let Some(p) = &prev {
                    if &k < p {
                        writeln!(stderr, "join: {name} is not in sorted order", name = files[idx]).unwrap();
                        return 2;
                    }
                }
                prev = Some(k);
            }
        }
    }

    let out_delim = delim.unwrap_or(' ');
    let start = header as usize;

    let mut map: BTreeMap<String, Vec<Vec<String>>> = BTreeMap::new();
    for line in l1.iter().skip(start) {
        let fields = split_fields(line);
        map.entry(key_of(&fields, f1)).or_default().push(fields);
    }

    let fmt_field = |fields: &[String], spec: &str| -> String {
        if spec == "0" {
            return fields.first().cloned().unwrap_or_default();
        }
        let dot = match spec.find('.') {
            Some(p) => p,
            None => { return String::new(); }
        };
        let _n = spec[..dot].parse::<usize>().unwrap_or(0);
        let m = spec[dot + 1..].parse::<usize>().unwrap_or(0);
        if m == 0 {
            return fields.first().cloned().unwrap_or_default();
        }
        fields.get(m - 1).cloned().unwrap_or_else(|| empty.clone())
    };

    if header {
        for line in [&l1, &l2] {
            if let Some(h) = line.first() {
                if stdout.write_all(h).is_err() {
                    return 1;
                }
                if stdout.write_all(&[term]).is_err() {
                    return 1;
                }
            }
        }
    }

    for line in l2.iter().skip(start) {
        let fields2 = split_fields(line);
        let key = key_of(&fields2, f2);
        let matches = map.get(&key).cloned().unwrap_or_default();
        if matches.is_empty() {
            if add2 || only2 {
                let rendered = if out_fmt.is_empty() {
                    fields2.join(&out_delim.to_string())
                } else {
                    out_fmt
                        .iter()
                        .map(|spec| {
                            if spec.starts_with('2') {
                                fmt_field(&fields2, spec)
                            } else if spec.starts_with('0') {
                                key.clone()
                            } else {
                                empty.clone()
                            }
                        })
                        .collect::<Vec<String>>()
                        .join(&out_delim.to_string())
                };
                if stdout.write_all(rendered.as_bytes()).is_err() {
                    return 1;
                }
                if stdout.write_all(&[term]).is_err() {
                    return 1;
                }
            }
            continue;
        }
        if only2 {
            continue;
        }
        for fields1 in &matches {
            let rendered = if out_fmt.is_empty() {
                let mut parts = vec![fields1.get(f1).cloned().unwrap_or_default()];
                for (i, f) in fields1.iter().enumerate() {
                    if i != f1 {
                        parts.push(f.clone());
                    }
                }
                for (i, f) in fields2.iter().enumerate() {
                    if i != f2 {
                        parts.push(f.clone());
                    }
                }
                parts.join(&out_delim.to_string())
            } else {
                out_fmt
                    .iter()
                    .map(|spec| {
                        if spec.starts_with('1') {
                            fmt_field(fields1, spec)
                        } else if spec.starts_with('2') {
                            fmt_field(&fields2, spec)
                        } else {
                            key.clone()
                        }
                    })
                    .collect::<Vec<String>>()
                    .join(&out_delim.to_string())
            };
            if stdout.write_all(rendered.as_bytes()).is_err() {
                return 1;
            }
            if stdout.write_all(&[term]).is_err() {
                return 1;
            }
        }
    }

    if add1 && !only1 {
        for (key, entries) in &map {
            let mut printed = false;
            for fields1 in entries {
                for line in l2.iter().skip(start) {
                    let f = split_fields(line);
                    if key_of(&f, f2) == *key {
                        printed = true;
                        break;
                    }
                }
                if printed {
                    continue;
                }
                let rendered = if out_fmt.is_empty() {
                    fields1.join(&out_delim.to_string())
                } else {
                    out_fmt
                        .iter()
                        .map(|spec| {
                            if spec.starts_with('1') {
                                fmt_field(fields1, spec)
                            } else if spec.starts_with('0') {
                                key.clone()
                            } else {
                                empty.clone()
                            }
                        })
                        .collect::<Vec<String>>()
                        .join(&out_delim.to_string())
                };
                if rendered.is_empty() {
                    continue;
                }
                if stdout.write_all(rendered.as_bytes()).is_err() {
                    return 1;
                }
                if stdout.write_all(&[term]).is_err() {
                    return 1;
                }
            }
        }
    }

    0
}
