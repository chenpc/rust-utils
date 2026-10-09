
use crate::In;
use crate::Out;

/// comm: compare two sorted files line by line.
pub fn run(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut suppress = [false; 3];
    let mut delim = "\t".to_string();
    let mut zero = false;
    let mut total = false;
    let mut check = true;
    let mut files: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--help" || a == "-h" {
            writeln!(stderr, "Usage: comm [OPTIONS] FILE1 FILE2").unwrap();
            return 0;
        }
        if a == "--version" || a == "-V" {
            writeln!(stderr, "comm (coreutils)").unwrap();
            return 0;
        }
        if a == "-1" || a == "-2" || a == "-3" {
            suppress[a[1..].parse::<usize>().unwrap() - 1] = true;
            i += 1;
            continue;
        }
        if a == "-z" || a == "--zero-terminated" {
            zero = true;
            i += 1;
            continue;
        }
        if a == "--total" {
            total = true;
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
        if a == "--output-delimiter" {
            if i + 1 >= args.len() {
                writeln!(stderr, "comm: missing argument after {a}").unwrap();
                return 2;
            }
            delim = args[i + 1].clone();
            i += 2;
            continue;
        }
        if a.starts_with('-') && a != "-" {
            writeln!(stderr, "comm: unknown option {a}").unwrap();
            return 2;
        }
        files.push(a.clone());
        i += 1;
    }

    if files.len() != 2 {
        writeln!(stderr, "comm: missing operand").unwrap();
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
                    writeln!(stderr, "comm: {name}: {e}").unwrap();
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

    let f1 = match read_all(&files[0]) {
        Ok(l) => l,
        Err(_) => return 1,
    };
    let f2 = match read_all(&files[1]) {
        Ok(l) => l,
        Err(_) => return 1,
    };

    if check {
        for (idx, lines) in [&f1, &f2].iter().enumerate() {
            for w in lines.windows(2) {
                if w[1] < w[0] {
                    writeln!(stderr, "comm: file {} is not in sorted order", idx + 1).unwrap();
                    return 2;
                }
            }
        }
    }

    let mut counts = [0usize; 3];
    let mut i1 = 0;
    let mut i2 = 0;
    while i1 < f1.len() || i2 < f2.len() {
        let col;
        if i1 >= f1.len() {
            col = 1;
            i2 += 1;
        } else if i2 >= f2.len() {
            col = 0;
            i1 += 1;
        } else if f1[i1] == f2[i2] {
            col = 2;
            i1 += 1;
            i2 += 1;
        } else if f1[i1] < f2[i2] {
            col = 0;
            i1 += 1;
        } else {
            col = 1;
            i2 += 1;
        }
        if suppress[col] {
            continue;
        }
        counts[col] += 1;
        let line = if col == 0 { &f1[i1 - 1] } else { &f2[i2 - 1] };
        let prefix = delim.repeat(col);
        if stdout.write_all(prefix.as_bytes()).is_err() {
            return 1;
        }
        if stdout.write_all(line).is_err() {
            return 1;
        }
        if stdout.write_all(&[term]).is_err() {
            return 1;
        }
    }

    if total {
        let parts: Vec<String> = counts.iter().map(|c| c.to_string()).collect();
        if stdout.write_all(parts.join(&delim).as_bytes()).is_err() {
            return 1;
        }
        if stdout.write_all(format!("{delim}total").as_bytes()).is_err() {
            return 1;
        }
        if stdout.write_all(&[term]).is_err() {
            return 1;
        }
    }
    0
}
