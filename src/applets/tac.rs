// tac: 把檔案（或 stdin）的記錄逆序輸出。
// man page: -b (separator before), -r (regex separator), -s STRING, -h, -V, [file]
// 假設：-r 支援最小正分子集（字面、.、*、+、?、[...]、|、\ 轉義），記錄在 docs/assumptions.md。

pub fn run(args: &[String], stdin: &mut dyn std::io::Read, stdout: &mut dyn std::io::Write, stderr: &mut dyn std::io::Write) -> i32 {
    let mut before = false;
    let mut use_regex = false;
    let mut sep = "\n".to_string();
    let mut files: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--help" || a == "-h" {
            writeln!(stderr, "Usage: tac [OPTION]... [FILE]...").unwrap();
            return 0;
        }
        if a == "--version" || a == "-V" {
            writeln!(stderr, "tac (coreutils) 0.1.0").unwrap();
            return 0;
        }
        if a.starts_with("--before") {
            before = true;
            i += 1;
            continue;
        }
        if a.starts_with("--regex") {
            use_regex = true;
            i += 1;
            continue;
        }
        if a.starts_with("--separator") {
            let (v, adv) = opt_value(args, i, a.strip_prefix("--separator").unwrap());
            sep = v;
            i += adv;
            continue;
        }
        if a.starts_with('-') && a.len() > 1 {
            let c = a[1..].chars().next().unwrap();
            match c {
                'b' => before = true,
                'r' => use_regex = true,
                's' => {
                    let (v, adv) = opt_value(args, i, &a[2..]);
                    sep = v;
                    i += adv;
                    continue;
                }
                _ => {
                    writeln!(stderr, "tac: unknown option -{c}").unwrap();
                    return 2;
                }
            }
            i += 1;
            continue;
        }
        files.push(a.clone());
        i += 1;
    }

    let mut status = 0;
    if files.is_empty() {
        let mut buf = Vec::new();
        stdin.read_to_end(&mut buf).unwrap();
        emit(&buf, &sep, use_regex, before, stdout);
        return 0;
    }
    for f in &files {
        if f == "-" {
            let mut buf = Vec::new();
            stdin.read_to_end(&mut buf).unwrap();
            emit(&buf, &sep, use_regex, before, stdout);
            continue;
        }
        match std::fs::read(f) {
            Ok(buf) => emit(&buf, &sep, use_regex, before, stdout),
            Err(e) => {
                writeln!(stderr, "tac: {f}: {e}").unwrap();
                status = 1;
            }
        }
    }
    status
}

fn opt_value(args: &[String], i: usize, rest: &str) -> (String, usize) {
    let rest = rest.trim_start_matches('=');
    if !rest.is_empty() {
        return (rest.to_string(), 1);
    }
    if i + 1 < args.len() {
        return (args[i + 1].clone(), 2);
    }
    (String::new(), 1)
}

fn emit(buf: &[u8], sep: &str, use_regex: bool, before: bool, out: &mut dyn std::io::Write) {
    // (記錄, 實際匹配到的分隔符)；-r 時輸出實際匹配的文字，不是模式本身
    let records = split_records(buf, sep, use_regex);
    for (r, m) in records.iter().rev() {
        if before {
            out.write_all(m).unwrap();
            out.write_all(r).unwrap();
        } else {
            out.write_all(r).unwrap();
            out.write_all(m).unwrap();
        }
    }
    out.flush().unwrap();
}

// 記錄 = 被分隔符切開的非空片段
fn split_records(buf: &[u8], sep: &str, use_regex: bool) -> Vec<(Vec<u8>, Vec<u8>)> {
    let sb = sep.as_bytes();
    if use_regex {
        return regex_split(buf, sep);
    }
    if sb.is_empty() {
        return vec![(buf.to_vec(), sb.to_vec())];
    }
    let mut recs = Vec::new();
    let mut start = 0;
    while start < buf.len() {
        let k = find_bytes(&buf[start..], sb);
        if let Some(k) = k {
            recs.push((buf[start..start + k].to_vec(), sb.to_vec()));
            start += k + sb.len();
        } else {
            recs.push((buf[start..].to_vec(), sb.to_vec()));
            break;
        }
    }
    recs.retain(|r| !r.0.is_empty());
    recs
}

fn regex_split(buf: &[u8], pat: &str) -> Vec<(Vec<u8>, Vec<u8>)> {
    let mut recs = Vec::new();
    let mut start = 0;
    let mut pos = 0;
    while pos < buf.len() {
        if let Some(n) = regex_find(pat, &buf[pos..]) {
            if n > 0 {
                recs.push((buf[start..pos].to_vec(), buf[pos..pos + n].to_vec()));
                pos += n;
                start = pos;
                continue;
            }
        }
        pos += 1;
    }
    if start < buf.len() {
        recs.push((buf[start..].to_vec(), Vec::new()));
    }
    recs.retain(|r| !r.0.is_empty());
    recs
}

fn find_bytes(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    for i in 0..=hay.len() - needle.len() {
        if hay[i..i + needle.len()] == needle[..] {
            return Some(i);
        }
    }
    None
}

// 最小正則：字面、.、*、+、?、[...]、|、\ 轉義；回傳從頭匹配的長度
fn regex_find(pat: &str, s: &[u8]) -> Option<usize> {
    for alt in pat.split('|') {
        if let Some(n) = match_alt(alt, s) {
            return Some(n);
        }
    }
    None
}

enum Atom {
    Ch(u8),
    Any,
    Set(Vec<u8>),
}

fn atom_class(p: &[u8], i: &mut usize) -> Atom {
    if p[*i] == b'\\' && *i + 1 < p.len() {
        let c = p[*i + 1];
        *i += 2;
        return Atom::Ch(c);
    }
    if p[*i] == b'.' {
        *i += 1;
        return Atom::Any;
    }
    if p[*i] == b'[' {
        let start = *i + 1;
        let mut end = start;
        while end < p.len() && p[end] != b']' {
            end += 1;
        }
        // 支援 a-z 範圍與字面字元
        let mut set: Vec<u8> = Vec::new();
        let mut j = start;
        while j < end {
            if j + 2 < end && p[j + 1] == b'-' {
                for c in p[j]..=p[j + 2] {
                    set.push(c);
                }
                j += 3;
            } else {
                set.push(p[j]);
                j += 1;
            }
        }
        *i = end + 1;
        return Atom::Set(set);
    }
    let c = p[*i];
    *i += 1;
    Atom::Ch(c)
}

fn atom_matches(a: &Atom, x: u8) -> bool {
    match a {
        Atom::Ch(c) => x == *c,
        Atom::Any => true,
        Atom::Set(s) => s.contains(&x),
    }
}

fn match_alt(alt: &str, s: &[u8]) -> Option<usize> {
    let p = alt.as_bytes();
    let mut i = 0;
    let mut used = 0;
    while i < p.len() {
        let atom = atom_class(p, &mut i);
        let mut min = 1usize;
        let mut max = 1usize;
        if i < p.len() {
            match p[i] {
                b'*' => { min = 0; max = usize::MAX; i += 1; }
                b'+' => { min = 1; max = usize::MAX; i += 1; }
                b'?' => { min = 0; max = 1; i += 1; }
                _ => {}
            }
        }
        let mut n = 0;
        while n < max && used < s.len() && atom_matches(&atom, s[used]) {
            n += 1;
            used += 1;
        }
        if n < min {
            return None;
        }
    }
    if used == 0 {
        return None;
    }
    Some(used)
}
