// paste: 把多個檔案的對應行以分隔符串起來。
// man page: -s (serial), -d LIST, -z (NUL 行分隔), -h, -V, [FILE]（預設 -）
// 假設：行以 \n 或 NUL 切開且結尾分隔符不產生空行；列輸出以行尾分隔符結束。

pub fn run(args: &[String], stdin: &mut dyn std::io::Read, stdout: &mut dyn std::io::Write, stderr: &mut dyn std::io::Write) -> i32 {
    let mut serial = false;
    let mut delims: Vec<u8> = Vec::new();
    let mut nul = false;
    let mut files: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--help" || a == "-h" {
            writeln!(stderr, "Usage: paste [OPTION]... [FILE]...").unwrap();
            return 0;
        }
        if a == "--version" || a == "-V" {
            writeln!(stderr, "paste (coreutils) 0.1.0").unwrap();
            return 0;
        }
        if a.starts_with("--serial") {
            serial = true;
            i += 1;
            continue;
        }
        if a.starts_with("--zero-terminated") {
            nul = true;
            i += 1;
            continue;
        }
        if a.starts_with("--delimiters") {
            let rest = a.strip_prefix("--delimiters").unwrap().trim_start_matches('=');
            if rest.is_empty() {
                if i + 1 >= args.len() {
                    writeln!(stderr, "paste: missing delimiter list").unwrap();
                    return 2;
                }
                delims.extend(args[i + 1].as_bytes());
                i += 2;
            } else {
                delims.extend(rest.as_bytes());
                i += 1;
            }
            continue;
        }
        if a.starts_with('-') && a.len() > 1 {
            let mut taken = false;
            for c in a[1..].chars() {
                match c {
                    's' => serial = true,
                    'z' => nul = true,
                    'd' => {
                        let rest = a[2..].to_string();
                        let val = if rest.is_empty() {
                            if i + 1 >= args.len() {
                                writeln!(stderr, "paste: missing delimiter list").unwrap();
                                return 2;
                            }
                            taken = true;
                            args[i + 1].clone()
                        } else {
                            rest
                        };
                        delims.extend(val.as_bytes());
                    }
                    _ => {
                        writeln!(stderr, "paste: unknown option -{c}").unwrap();
                        return 2;
                    }
                }
            }
            if taken {
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        files.push(a.clone());
        i += 1;
    }
    if files.is_empty() {
        files.push("-".to_string());
    }
    if delims.is_empty() {
        delims.push(b'\t');
    }
    let term = if nul { b'\0' } else { b'\n' };

    if serial {
        let mut status = 0;
        for f in &files {
            let data = if f == "-" {
                let mut b = Vec::new();
                stdin.read_to_end(&mut b).unwrap();
                b
            } else {
                match std::fs::read(f) {
                    Ok(b) => b,
                    Err(e) => {
                        writeln!(stderr, "paste: {f}: {e}").unwrap();
                        status = 1;
                        continue;
                    }
                }
            };
            let ls = lines(&data, term);
            for (k, line) in ls.iter().enumerate() {
                if k > 0 {
                    stdout.write_all(&[delims[(k - 1) % delims.len()]]).unwrap();
                }
                stdout.write_all(line).unwrap();
            }
            if !ls.is_empty() {
                stdout.write_all(&[term]).unwrap();
            }
        }
        return status;
    }

    let mut cols: Vec<Vec<Vec<u8>>> = Vec::new();
    let mut status = 0;
    for f in &files {
        let data = if f == "-" {
            let mut b = Vec::new();
            stdin.read_to_end(&mut b).unwrap();
            b
        } else {
            match std::fs::read(f) {
                Ok(b) => b,
                Err(e) => {
                    writeln!(stderr, "paste: {f}: {e}").unwrap();
                    status = 1;
                    continue;
                }
            }
        };
        cols.push(lines(&data, term));
    }
    let nf = cols.len();
    let nrows = cols.iter().map(|c| c.len()).max().unwrap_or(0);
    for r in 0..nrows {
        for k in 0..nf {
            if k > 0 {
                stdout.write_all(&[delims[(k - 1) % delims.len()]]).unwrap();
            }
            stdout.write_all(&cols[k].get(r).cloned().unwrap_or_default()).unwrap();
        }
        stdout.write_all(&[term]).unwrap();
    }
    status
}

fn lines(data: &[u8], term: u8) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut start = 0;
    while start < data.len() {
        match data[start..].iter().position(|&b| b == term) {
            Some(k) => {
                out.push(data[start..start + k].to_vec());
                start += k + 1;
            }
            None => {
                out.push(data[start..].to_vec());
                break;
            }
        }
    }
    out
}
