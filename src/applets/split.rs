use std::process::Command;

use crate::In;
use crate::Out;

/// split: create output files containing consecutive or interleaved sections of input.
pub fn run(args: &[String], stdin: In, _stdout: Out, stderr: Out) -> i32 {
    let mut bytes: Option<usize> = None;
    let mut line_bytes: Option<usize> = None;
    let mut lines_per = 1000usize;
    let mut chunks: Option<String> = None;
    let mut extra = String::new();
    let mut filter = String::new();
    let mut elide = false;
    let mut suffix_mode = "alpha".to_string();
    let mut from = 0usize;
    let mut slen = 2usize;
    let mut verbose = false;
    let mut sep = b'\n';
    let mut input = String::new();
    let mut prefix = String::new();

    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--help" || a == "-h" {
            writeln!(stderr, "Usage: split [OPTIONS] [INPUT] [PREFIX]").unwrap();
            return 0;
        }
        if a == "--version" || a == "-V" {
            writeln!(stderr, "split (coreutils)").unwrap();
            return 0;
        }
        if a == "-e" || a == "--elide-empty-files" {
            elide = true;
            i += 1;
            continue;
        }
        if a == "-d" || a == "--numeric-suffixes" {
            suffix_mode = "numeric".to_string();
            if let Some(v) = a.split_once('=') {
                from = v.1.parse::<usize>().unwrap_or(0);
            } else if i + 1 < args.len() && args[i + 1].starts_with('=') {
                from = args[i + 1][1..].parse::<usize>().unwrap_or(0);
                i += 1;
            }
            i += 1;
            continue;
        }
        if a == "-x" || a == "--hex-suffixes" {
            suffix_mode = "hex".to_string();
            if let Some(v) = a.split_once('=') {
                from = v.1.parse::<usize>().unwrap_or(0);
            }
            i += 1;
            continue;
        }
        if a == "--verbose" {
            verbose = true;
            i += 1;
            continue;
        }
        if a == "--additional-suffix" || a == "--filter" || a == "-a" || a == "--suffix-length" || a == "-t" || a == "--separator" {
            if i + 1 >= args.len() {
                writeln!(stderr, "split: missing argument after {a}").unwrap();
                return 2;
            }
            let v = args[i + 1].clone();
            match a.as_str() {
                "--additional-suffix" => extra = v,
                "--filter" => filter = v,
                "-a" | "--suffix-length" => slen = v.parse::<usize>().unwrap_or(2),
                _ => sep = v.as_bytes()[0],
            }
            i += 2;
            continue;
        }
        if a == "-b" || a == "--bytes" || a == "-C" || a == "--line-bytes" || a == "-l" || a == "--lines" || a == "-n" || a == "--number" {
            if i + 1 >= args.len() {
                writeln!(stderr, "split: missing argument after {a}").unwrap();
                return 2;
            }
            let v = args[i + 1].clone();
            match a.as_str() {
                "-b" | "--bytes" => bytes = v.parse::<usize>().ok(),
                "-C" | "--line-bytes" => line_bytes = v.parse::<usize>().ok(),
                "-l" | "--lines" => lines_per = v.parse::<usize>().unwrap_or(1000),
                _ => chunks = Some(v),
            }
            i += 2;
            continue;
        }
        if a.starts_with('-') && a != "-" {
            writeln!(stderr, "split: unknown option {a}").unwrap();
            return 2;
        }
        if input.is_empty() && prefix.is_empty() {
            input = a.clone();
        } else {
            prefix = a.clone();
        }
        i += 1;
    }

    if prefix.is_empty() {
        prefix = "x".to_string();
    }

    let data = if input.is_empty() || input == "-" {
        let mut d = Vec::new();
        if stdin.read_to_end(&mut d).is_err() {
            return 1;
        }
        d
    } else {
        match std::fs::read(&input) {
            Ok(d) => d,
            Err(e) => {
                writeln!(stderr, "split: {input}: {e}").unwrap();
                return 1;
            }
        }
    };

    let mut records: Vec<Vec<u8>> = {
        let mut r = Vec::new();
        let mut start = 0;
        while start < data.len() {
            let end = match data[start..].iter().position(|&c| c == sep) {
                Some(p) => start + p + 1,
                None => data.len(),
            };
            r.push(data[start..end].to_vec());
            start = end;
        }
        r
    };

    // group records into chunks
    let mut groups: Vec<Vec<Vec<u8>>> = Vec::new();
    if let Some(c) = &chunks {
        let (mode, n) = match c.split_once('/') {
            Some((m, n)) => (m.to_string(), n.parse::<usize>().unwrap_or(0)),
            None => ("".to_string(), c.parse::<usize>().unwrap_or(0)),
        };
        if n == 0 {
            writeln!(stderr, "split: invalid number of chunks: {c}").unwrap();
            return 2;
        }
        if mode == "k" {
            // round-robin by record
            for k in 0..n {
                let mut g = Vec::new();
                for (idx, rec) in records.iter().enumerate() {
                    if idx % n == k {
                        g.push(rec.clone());
                    }
                }
                groups.push(g);
            }
        } else if mode == "r" {
            let total: usize = records.iter().map(|r| r.len()).sum();
            let base = total / n;
            let rem = total % n;
            let mut pos = 0;
            for k in 0..n {
                let want = base + if k < rem { 1 } else { 0 };
                let mut g = Vec::new();
                let mut used = 0;
                while used < want && pos < records.len() {
                    let rec = &records[pos];
                    if used + rec.len() <= want || rec.len() <= want - used {
                        g.push(rec.clone());
                        used += rec.len();
                        pos += 1;
                    } else {
                        let take = want - used;
                        g.push(rec[..take].to_vec());
                        records[pos] = rec[take..].to_vec();
                        used += take;
                    }
                }
                groups.push(g);
            }
        } else {
            let total: usize = data.len();
            let size = (total + n - 1) / n;
            if size == 0 {
                groups.push(Vec::new());
            } else {
                let mut pos = 0;
                while pos < data.len() {
                    groups.push(vec![data[pos..(pos + size).min(data.len())].to_vec()]);
                    pos += size;
                }
            }
        }
    } else if let Some(b) = bytes {
        let mut pos = 0;
        while pos < data.len() {
            groups.push(vec![data[pos..(pos + b).min(data.len())].to_vec()]);
            pos += b;
        }
    } else if let Some(b) = line_bytes {
        let mut pos = 0;
        while pos < records.len() {
            let mut g = Vec::new();
            let mut used = 0;
            while pos < records.len() && used + records[pos].len() <= b {
                g.push(records[pos].clone());
                used += records[pos].len();
                pos += 1;
            }
            if g.is_empty() && pos < records.len() {
                g.push(records[pos].clone());
                pos += 1;
            }
            groups.push(g);
        }
    } else {
        let mut pos = 0;
        while pos < records.len() {
            groups.push(records[pos..(pos + lines_per).min(records.len())].to_vec());
            pos += lines_per;
        }
    }

    if groups.is_empty() {
        groups.push(Vec::new());
    }

    for (idx, g) in groups.iter().enumerate() {
        let content: Vec<u8> = g.concat();
        if elide && content.is_empty() {
            continue;
        }
        let name = format!("{prefix}{}{extra}", suffix(idx + from, slen, &suffix_mode));
        if verbose {
            writeln!(stderr, "writing to '{name}'").unwrap();
        }
        if !filter.is_empty() {
            let rc = Command::new("sh")
                .arg("-c")
                .arg(&filter)
                .env("FILE", &name)
                .status()
                .map(|s| s.code().unwrap_or(1))
                .unwrap_or(1);
            if rc != 0 {
                writeln!(stderr, "split: filter failed for {name}").unwrap();
                return 1;
            }
            continue;
        }
        if std::fs::write(&name, &content).is_err() {
            writeln!(stderr, "split: cannot write {name}").unwrap();
            return 1;
        }
    }
    0
}

fn suffix(idx: usize, len: usize, mode: &str) -> String {
    match mode {
        "numeric" => format!("{idx:0>len$}"),
        "hex" => format!("{idx:0>len$x}"),
        _ => {
            let mut s = String::new();
            let mut n = idx;
            for _ in 0..len {
                s.insert(0, (b'a' + (n % 26) as u8) as char);
                n /= 26;
            }
            s
        }
    }
}
