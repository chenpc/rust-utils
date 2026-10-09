
use crate::In;
use crate::Out;

use super::expand::{char_width, next_stop, tab_stops};

/// unexpand: convert blanks to tabs.
pub fn run(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut all = false;
    let mut first_only = false;
    let mut stops = vec![8usize];
    let mut utf8 = true;
    let mut files: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--help" || a == "-h" {
            writeln!(stderr, "Usage: unexpand [OPTIONS] [FILE]").unwrap();
            return 0;
        }
        if a == "--version" || a == "-V" {
            writeln!(stderr, "unexpand (coreutils)").unwrap();
            return 0;
        }
        if a == "-a" || a == "--all" {
            all = true;
            i += 1;
            continue;
        }
        if a == "-f" || a == "--first-only" {
            first_only = true;
            i += 1;
            continue;
        }
        if a == "-U" || a == "--no-utf8" {
            utf8 = false;
            i += 1;
            continue;
        }
        if a == "-t" || a == "--tabs" {
            if i + 1 >= args.len() {
                writeln!(stderr, "unexpand: missing argument after {a}").unwrap();
                return 2;
            }
            stops = tab_stops(&args[i + 1]);
            all = true; // -t enables -a per man page
            i += 2;
            continue;
        }
        if a.starts_with('-') && a != "-" {
            writeln!(stderr, "unexpand: unknown option {a}").unwrap();
            return 2;
        }
        files.push(a.clone());
        i += 1;
    }

    if stops.is_empty() {
        stops = vec![8];
    }

    let mut status = 0;
    let sources: Vec<String> = if files.is_empty() {
        vec!["-".to_string()]
    } else {
        files
    };
    for f in sources {
        let data = if f == "-" {
            let mut d = Vec::new();
            if stdin.read_to_end(&mut d).is_err() {
                return 1;
            }
            d
        } else {
            match std::fs::read(&f) {
                Ok(d) => d,
                Err(e) => {
                    writeln!(stderr, "unexpand: {f}: {e}").unwrap();
                    status = 1;
                    continue;
                }
            }
        };

        let mut pos = 0;
        while pos < data.len() {
            let end = match data[pos..].iter().position(|&c| c == b'\n') {
                Some(p) => pos + p,
                None => data.len(),
            };
            let raw = &data[pos..end];
            let mut col = 0usize;
            let mut j = 0;
            let mut leading = true;
            while j < raw.len() {
                let b = raw[j];
                if b == b' ' {
                    let mut run = 0;
                    while j + run < raw.len() && raw[j + run] == b' ' {
                        run += 1;
                    }
                    // Default is first-only; -a converts everywhere; -f overrides -a.
                    let eligible = leading || (all && !first_only);
                    if eligible {
                        let mut remaining = run;
                        while remaining > 0 {
                            let target = next_stop(col, &stops);
                            if target > col + remaining {
                                break;
                            }
                            if stdout.write_all(b"\t").is_err() {
                                return 1;
                            }
                            remaining -= target - col;
                            col = target;
                        }
                        if remaining > 0 {
                            let sp = " ".repeat(remaining);
                            if stdout.write_all(sp.as_bytes()).is_err() {
                                return 1;
                            }
                            col += remaining;
                        }
                        j += run;
                        leading = false;
                        continue;
                    }
                    if stdout.write_all(&raw[j..j + run]).is_err() {
                        return 1;
                    }
                    col += run;
                    j += run;
                    leading = false;
                    continue;
                }
                if b == b'\t' {
                    if stdout.write_all(b"\t").is_err() {
                        return 1;
                    }
                    col = next_stop(col, &stops);
                    if !leading {
                        leading = false;
                    }
                    j += 1;
                    continue;
                }
                let w = char_width(b);
                if stdout.write_all(&raw[j..j + w]).is_err() {
                    return 1;
                }
                col += if utf8 { 1 } else { w };
                j += w;
                leading = false;
            }
            if end < data.len() {
                if stdout.write_all(b"\n").is_err() {
                    return 1;
                }
            }
            pos = end + 1;
        }
    }
    status
}
