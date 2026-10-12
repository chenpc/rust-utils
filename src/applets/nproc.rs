use crate::{In, Out};

const NPROC_HELP: &str = r#"Print the number of cores available to the current process.

Usage: nproc [OPTION]...

Options:
      --all              print the number of cores available to the system
      --ignore <N>       ignore up to N cores [default: 0]
  -h, --help             Print help
  -V, --version          Print version

Print the number of cores available to the current process.
If the OMP_NUM_THREADS or OMP_THREAD_LIMIT environment variables are set, then
they will determine the minimum and maximum returned value respectively.
"#;

fn tail(stderr: Out) -> i32 {
    write!(stderr, "\n\nFor more information, try '--help'.\n").ok();
    let _ = crate::pathname::usage_hint("nproc");
    1
}

fn parse_count(s: &str) -> Option<usize> {
    if s.is_empty() {
        return None;
    }
    s.parse::<usize>().ok()
}

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut all = false;
    let mut ignore: usize = 0;
    let mut ignore_given = false;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        i += 1;
        if a == "--" {
            continue;
        }
        if a == "--help" || a == "-h" || a == "-hV" {
            write!(stdout, "{}", NPROC_HELP).ok();
            return 0;
        }
        if a == "--version" || a == "-V" || a == "-Vh" {
            writeln!(stdout, "nproc (uutils coreutils) 0.10.0").ok();
            return 0;
        }
        if a.starts_with("--ignore") {
            if ignore_given {
                write!(stderr, "error: the argument '--ignore <N>' cannot be used multiple times").ok();
                return tail(stderr);
            }
            ignore_given = true;
            let inline = a.split_once('=');
            if let Some((_, value)) = inline {
                match parse_count(value) {
                    Some(n) => ignore = n,
                    None => {
                        write!(stderr, 
                            "error: invalid value '{}' for '--ignore <N>': invalid digit found in string",
                            value
                        ).ok();
                        return tail(stderr);
                    }
                }
                continue;
            }
            if a != "--ignore" {
                return unexpected(a, stderr);
            }
            if i >= args.len() {
                write!(stderr, "error: a value is required for '--ignore <N>' but none was supplied").ok();
                return tail(stderr);
            }
            let value = args[i].clone();
            i += 1;
            match parse_count(&value) {
                Some(n) => ignore = n,
                None => {
                    write!(stderr, 
                        "error: invalid value '{}' for '--ignore <N>': invalid digit found in string",
                        value
                    ).ok();
                    return tail(stderr);
                }
            }
            continue;
        }
        if a == "--all" {
            if all {
                write!(stderr, "error: the argument '--all' cannot be used multiple times").ok();
                return tail(stderr);
            }
            all = true;
            continue;
        }
        if a.starts_with("--") {
            return unexpected(a, stderr);
        }
        if a.starts_with('-') && a.len() > 1 {
            for k in 1..a.len() {
                match a.as_bytes()[k] {
                    b'h' => {
                        write!(stdout, "{}", NPROC_HELP).ok();
                        return 0;
                    }
                    b'V' => {
                        writeln!(stdout, "nproc (uutils coreutils) 0.10.0").ok();
                        return 0;
                    }
                    _ => {
                        write!(stderr, 
                            "error: unexpected argument '-{}' found",
                            &a[k..k + 1]
                        ).ok();
                        return tail(stderr);
                    }
                }
            }
            continue;
        }
        write!(stderr, "error: unexpected argument '{}' found", a).ok();
        return tail(stderr);
    }

    let base: usize = if all {
        crate::sys::present_cpu_count().unwrap_or_else(|| crate::sys::affinity_count().unwrap_or(1))
    } else {
        crate::sys::affinity_count()
            .or_else(crate::sys::present_cpu_count)
            .unwrap_or(1)
    };

    // OMP_NUM_THREADS replaces the count (it is the lower bound named in the man
    // page); OMP_THREAD_LIMIT caps it afterwards.  Zero or unparsable values mean
    // "not set".
    let mut count = base;
    if !all {
        if let Ok(v) = std::env::var("OMP_NUM_THREADS") {
            if let Some(n) = parse_count(&v) {
                if n > 0 {
                    count = n;
                }
            }
        }
    }
    count = count.saturating_sub(ignore);
    if count < 1 {
        count = 1;
    }
    if let Ok(v) = std::env::var("OMP_THREAD_LIMIT") {
        if let Some(n) = parse_count(&v) {
            if n > 0 && n < count {
                count = n;
            }
        }
    }
    writeln!(stdout, "{}", count).ok();
    0
}

fn unexpected(arg: &str, stderr: Out) -> i32 {
    write!(stderr, "error: unexpected argument '{}' found", arg).ok();
    tail(stderr)
}
