pub struct Cfg {
    sep: String,
    term: String,
    format: Option<String>,
    equal_width: bool,
}

pub fn run(args: &[String], stdin: &mut dyn std::io::Read, stdout: &mut dyn std::io::Write, stderr: &mut dyn std::io::Write) -> i32 {
    let _ = stdin;
    let mut cfg = Cfg { sep: "\n".into(), term: "\n".into(), format: None, equal_width: false };
    let mut nums: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--help" || a == "-h" {
            writeln!(stderr, "Usage: seq [OPTION]... FIRST [INCREMENT] LAST").unwrap();
            return 0;
        }
        if a == "--version" || a == "-V" {
            writeln!(stderr, "seq (coreutils) 0.1.0").unwrap();
            return 0;
        }
        if is_number(a) || !a.starts_with('-') || a.len() == 1 {
            nums.push(a.clone());
            i += 1;
            continue;
        }
        // long options, possibly with =value
        if let Some(rest) = a.strip_prefix("--separator") {
            let (v, adv) = opt_value(args, i, rest);
            cfg.sep = unescape(&v);
            i += adv;
            continue;
        }
        if let Some(rest) = a.strip_prefix("--terminator") {
            let (v, adv) = opt_value(args, i, rest);
            cfg.term = unescape(&v);
            i += adv;
            continue;
        }
        if let Some(rest) = a.strip_prefix("--format") {
            let (v, adv) = opt_value(args, i, rest);
            cfg.format = Some(v);
            i += adv;
            continue;
        }
        if a == "--equal-width" {
            cfg.equal_width = true;
            i += 1;
            continue;
        }
        let c = a[1..].chars().next().unwrap();
        let (v, adv) = if a.len() == 2 {
            (args.get(i + 1).cloned().unwrap_or_default(), 2)
        } else {
            (a[2..].to_string(), 1)
        };
        match c {
            'w' => {
                cfg.equal_width = true;
                i += 1;
                continue;
            }
            's' => cfg.sep = unescape(&v),
            't' => cfg.term = unescape(&v),
            'f' => cfg.format = Some(v),
            _ => {
                writeln!(stderr, "seq: unknown option -{c}").unwrap();
                return 2;
            }
        }
        i += adv;
    }

    if nums.is_empty() {
        writeln!(stderr, "seq: missing operand").unwrap();
        return 2;
    }

    let (first, incr, last) = match nums.len() {
        1 => (1.0, 1.0, parse(&nums[0])),
        2 => (parse(&nums[0]), 1.0, parse(&nums[1])),
        _ => (parse(&nums[0]), parse(&nums[1]), parse(&nums[2])),
    };
    if incr == 0.0 {
        writeln!(stderr, "seq: invalid increment: 0").unwrap();
        return 1;
    }

    let strs: Vec<String> = nums.iter().map(|s| fmt_num(s, cfg.format.as_deref())).collect();
    let w = if cfg.equal_width { strs.iter().map(|s| s.len()).max().unwrap_or(0) } else { 0 };

    let mut n = first;
    let mut first_out = true;
    while (incr > 0.0 && n <= last) || (incr < 0.0 && n >= last) {
        if !first_out {
            write!(stdout, "{}", cfg.sep).unwrap();
        }
        first_out = false;
        let s = fmt_num(&num_to_string(n), cfg.format.as_deref());
        if w > 0 && s.len() < w {
            write!(stdout, "{}{}", "0".repeat(w - s.len()), s).unwrap();
        } else {
            write!(stdout, "{}", s).unwrap();
        }
        n += incr;
    }
    write!(stdout, "{}", cfg.term).unwrap();
    stdout.flush().unwrap();
    0
}

fn is_number(s: &str) -> bool {
    s.parse::<f64>().ok().is_some()
}

// returns (value, how many args to advance)
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

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' && i + 1 < b.len() {
            match b[i + 1] {
                b'n' => out.push('\n'),
                b't' => out.push('\t'),
                b'r' => out.push('\r'),
                b'\\' => out.push('\\'),
                c => out.push(c as char),
            }
            i += 2;
        } else {
            out.push(b[i] as char);
            i += 1;
        }
    }
    out
}

fn parse(s: &str) -> f64 {
    s.parse::<f64>().unwrap_or(0.0)
}

fn num_to_string(n: f64) -> String {
    if n == n.floor() && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    }
}

// minimal printf-style format: %[width][.precision]conv where conv in g d f e s
fn fmt_num(s: &str, fmt: Option<&str>) -> String {
    match fmt {
        None => s.to_string(),
        Some(f) => {
            let n = parse(s);
            let b = f.as_bytes();
            let mut out = String::new();
            let mut i = 0;
            while i < b.len() {
                if b[i] != b'%' {
                    out.push(b[i] as char);
                    i += 1;
                    continue;
                }
                i += 1;
                if i >= b.len() {
                    out.push('%');
                    break;
                }
                let mut width = 0usize;
                let mut prec: Option<usize> = None;
                while i < b.len() && b[i].is_ascii_digit() {
                    width = width * 10 + (b[i] - b'0') as usize;
                    i += 1;
                }
                if i < b.len() && b[i] == b'.' {
                    i += 1;
                    let mut p = 0usize;
                    while i < b.len() && b[i].is_ascii_digit() {
                        p = p * 10 + (b[i] - b'0') as usize;
                        i += 1;
                    }
                    prec = Some(p);
                }
                if i >= b.len() {
                    break;
                }
                let conv = b[i];
                i += 1;
                let s = match conv {
                    b'g' => match prec {
                        Some(p) => format!("{:.prec$}", n, prec = p),
                        None => format!("{}", n),
                    },
                    b'd' => format!("{}", n as i64),
                    b'f' => match prec {
                        Some(p) => format!("{:.prec$}", n, prec = p),
                        None => format!("{:.6}", n),
                    },
                    b'e' => format!("{:e}", n),
                    b's' => s.to_string(),
                    c => (c as char).to_string(),
                };
                if width > s.len() {
                    out.push_str(&" ".repeat(width - s.len()));
                }
                out.push_str(&s);
            }
            out
        }
    }
}
