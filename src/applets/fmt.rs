// fmt — reformat paragraphs from input (or standard input) to stdout.
//
// Behavior model (docs/man/fmt.1.gz):
//   -c crown margin, -t tagged paragraph, -m preserve headers, -s split only,
//   -u uniform spacing, -p PREFIX, -P PSKIP, -x exact prefix, -X exact skip prefix,
//   -w WIDTH (default 75), -g GOAL (default 93% of WIDTH), -q quick,
//   -T TABWIDTH (default 8), -h, -V, [FILES].
//
// Assumptions (docs/assumptions.md):
//   - a paragraph is a run of consecutive non-empty lines; an empty line ends a paragraph
//     and is passed through;
//   - the paragraph prefix is the leading whitespace of its first line (or -p/-P);
//   - reflow is greedy: append words until the next word would exceed WIDTH, then flush;
//     a word longer than WIDTH cannot be broken, so that line is longer than WIDTH;
//   - -q flushes as soon as the current line reaches GOAL;
//   - -u: exactly one space between words, two after a sentence end, where a sentence end is
//     [?!.] followed by two spaces or a newline in the input;
//   - a header line (for -m) is a line whose first word ends with ':' → output unchanged;
//   - -t: if the first two lines of a paragraph have the same indentation they become two
//     separate paragraphs;
//   - -c: the first line keeps its own indentation, later lines use the second line's;
//   - tabs are preserved in output (they only count as TABWIDTH for line length);
//   - the prefix (-p) is stripped from the line body before reflowing.

use std::io::{Read, Write};

struct Opts<'a> {
    crown: bool,
    tagged: bool,
    headers: bool,
    split_only: bool,
    uniform: bool,
    prefix: &'a str,
    skip_prefix: &'a str,
    exact_prefix: bool,
    exact_skip: bool,
    width: usize,
    goal: usize,
    quick: bool,
    tab_width: usize,
}

struct Tok {
    sep: String,
    word: String,
}

fn dlen(s: &str, tab_width: usize) -> usize {
    let mut n = 0;
    for c in s.chars() {
        n += if c == '\t' { tab_width } else { 1 };
    }
    n
}

fn indent_of(line: &str) -> String {
    let mut out = String::new();
    for c in line.chars() {
        if c == ' ' || c == '\t' {
            out.push(c);
        } else {
            break;
        }
    }
    out
}

fn is_header(line: &str) -> bool {
    line.split_whitespace().next().unwrap_or("").ends_with(':')
}

fn matches_prefix(line: &str, p: &str, exact: bool) -> bool {
    if p.is_empty() {
        return false;
    }
    if exact {
        line.starts_with(p)
    } else {
        line.trim_start().starts_with(p)
    }
}

fn sentence_end(line: &str) -> bool {
    let t = line.trim_end();
    match t.chars().last() {
        Some(c) => ".?!".contains(c),
        None => false,
    }
}

// tokens of one line: (separator before the word, word); the first token's separator is the
// line's indentation
fn tokenize(line: &str) -> Vec<Tok> {
    let b = line.as_bytes();
    let mut out: Vec<Tok> = Vec::new();
    let mut i = 0;
    let mut sep = String::new();
    while i < b.len() {
        let c = b[i] as char;
        if c.is_whitespace() {
            sep.push(c);
            i += 1;
            continue;
        }
        let start = i;
        while i < b.len() && !(b[i] as char).is_whitespace() {
            i += 1;
        }
        out.push(Tok {
            sep,
            word: line[start..i].to_string(),
        });
        sep = String::new();
    }
    out
}

fn emit(para: &[String], stdout: &mut dyn Write, o: &Opts) -> i32 {
    if !o.skip_prefix.is_empty() || !o.prefix.is_empty() {
        for l in para {
            if !o.skip_prefix.is_empty() && matches_prefix(l, o.skip_prefix, o.exact_skip) {
                if stdout.write_all(format!("{}\n", l).as_bytes()).is_err() {
                    return 1;
                }
                continue;
            }
            if !o.prefix.is_empty() && !matches_prefix(l, o.prefix, o.exact_prefix) {
                if stdout.write_all(format!("{}\n", l).as_bytes()).is_err() {
                    return 1;
                }
                continue;
            }
            let p = if o.prefix.is_empty() { indent_of(l) } else { o.prefix.to_string() };
            let body = if !o.prefix.is_empty() && matches_prefix(l, o.prefix, o.exact_prefix) {
                &l[o.prefix.len()..]
            } else {
                l
            };
            let toks = tokenize(body);
            if wrap(&toks, &p, stdout, o) == 1 {
                return 1;
            }
        }
        return 0;
    }

    if o.tagged && para.len() >= 2 && indent_of(&para[0]) == indent_of(&para[1]) {
        for l in para {
            if emit(std::slice::from_ref(l), stdout, o) == 1 {
                return 1;
            }
        }
        return 0;
    }

    let first_indent = indent_of(&para[0]);
    let base = if o.crown && para.len() > 1 {
        indent_of(&para[1])
    } else {
        first_indent.clone()
    };

    if o.split_only {
        for l in para {
            let p = if o.crown { indent_of(l) } else { base.clone() };
            if wrap(&tokenize(l), &p, stdout, o) == 1 {
                return 1;
            }
        }
        return 0;
    }

    // crown: the first line keeps its own indentation
    if o.crown && para.len() > 1 {
        let toks = tokenize(&para[0]);
        if !toks.is_empty() {
            let line = format!("{}{}", first_indent, toks.iter().map(|t| t.word.as_str()).collect::<Vec<&str>>().join(" "));
            if stdout.write_all(format!("{}\n", line).as_bytes()).is_err() {
                return 1;
            }
        }
        let rest: Vec<String> = para[1..].to_vec();
        return emit(&rest, stdout, o);
    }

    let mut toks: Vec<Tok> = Vec::new();
    let mut pending_double = false;
    for (idx, l) in para.iter().enumerate() {
        if o.headers && is_header(l) {
            if stdout.write_all(format!("{}\n", l).as_bytes()).is_err() {
                return 1;
            }
            continue;
        }
        for (k, t) in tokenize(l).into_iter().enumerate() {
            let is_line_start = k == 0 && !l.starts_with(' ') && !l.starts_with('\t');
            let sep = if o.uniform {
                if pending_double || t.sep.len() >= 2 {
                    "  ".to_string()
                } else {
                    " ".to_string()
                }
            } else if k == 0 && idx > 0 && !is_line_start {
                " ".to_string()
            } else if k == 0 && idx > 0 {
                " ".to_string()
            } else {
                t.sep.clone()
            };
            pending_double = false;
            toks.push(Tok { sep, word: t.word });
        }
        if o.uniform && sentence_end(l) {
            pending_double = true;
        }
        let _ = idx;
    }
    if toks.is_empty() {
        return 0;
    }
    let mut cur = String::new();
    let mut start_of_line = true;
    for t in toks.iter() {
        if start_of_line {
            cur = format!("{}{}", base, t.word);
            start_of_line = false;
            continue;
        }
        let sep = t.sep.clone();
        let candidate = format!("{}{}{}", cur, sep, t.word);
        if dlen(&candidate, o.tab_width) > o.width {
            if stdout.write_all(format!("{}\n", cur).as_bytes()).is_err() {
                return 1;
            }
            cur = format!("{}{}", base, t.word);
        } else {
            cur = candidate;
            if o.quick && dlen(&cur, o.tab_width) >= o.goal {
                if stdout.write_all(format!("{}\n", cur).as_bytes()).is_err() {
                    return 1;
                }
                cur = base.clone();
                start_of_line = true;
            }
        }
    }
    if !cur.is_empty() && cur != base.clone() {
        if stdout.write_all(format!("{}\n", cur).as_bytes()).is_err() {
            return 1;
        }
    }
    0
}

fn wrap(toks: &[Tok], prefix: &str, stdout: &mut dyn Write, o: &Opts) -> i32 {
    if toks.is_empty() {
        return 0;
    }
    let mut cur = format!("{}{}{}", prefix, toks[0].sep, toks[0].word);
    for t in &toks[1..] {
        let sep = if o.uniform { " " } else { t.sep.as_str() };
        let candidate = format!("{}{}{}", cur, sep, t.word);
        if dlen(&candidate, o.tab_width) > o.width {
            if stdout.write_all(format!("{}\n", cur).as_bytes()).is_err() {
                return 1;
            }
            cur = format!("{}{}", prefix, t.word);
        } else {
            cur = candidate;
        }
    }
    if stdout.write_all(format!("{}\n", cur).as_bytes()).is_err() {
        return 1;
    }
    0
}

pub fn run(args: &[String], stdin: &mut dyn Read, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    let mut o = Opts {
        crown: false,
        tagged: false,
        headers: false,
        split_only: false,
        uniform: false,
        prefix: "",
        skip_prefix: "",
        exact_prefix: false,
        exact_skip: false,
        width: 75,
        goal: 0,
        quick: false,
        tab_width: 8,
    };
    let mut files: Vec<String> = Vec::new();
    let mut prefix_owned = String::new();
    let mut skip_owned = String::new();
    let mut i = 0;
    let mut no_more = false;
    while i < args.len() {
        let a = args[i].clone();
        if no_more || !a.starts_with('-') {
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
        if a == "-h" || a == "--help" {
            if stdout.write_all(b"fmt: reformat paragraphs to stdout\n").is_err() {
                return 1;
            }
            return 0;
        }
        if a == "-V" || a == "--version" {
            if stdout.write_all(b"fmt (uutils coreutils) 0.10.0\n").is_err() {
                return 1;
            }
            return 0;
        }
        let takes_value = matches!(
            a.as_str(),
            "-p" | "--prefix" | "-P" | "--skip-prefix" | "-w" | "--width" | "-g" | "--goal" | "-T" | "--tab-width"
        );
        if takes_value && i + 1 >= args.len() {
            let _ = stderr.write_all(format!("fmt: missing operand after '{}'\n", a).as_bytes());
            return 2;
        }
        let value = if i + 1 < args.len() { args[i + 1].clone() } else { String::new() };
        match a.as_str() {
            "-c" | "--crown-margin" => o.crown = true,
            "-t" | "--tagged-paragraph" => o.tagged = true,
            "-m" | "--preserve-headers" => o.headers = true,
            "-s" | "--split-only" => o.split_only = true,
            "-u" | "--uniform-spacing" => o.uniform = true,
            "-x" | "--exact-prefix" => o.exact_prefix = true,
            "-X" | "--exact-skip-prefix" => o.exact_skip = true,
            "-q" | "--quick" => o.quick = true,
            "-p" | "--prefix" => {
                prefix_owned = value;
                i += 1;
            }
            "-P" | "--skip-prefix" => {
                skip_owned = value;
                i += 1;
            }
            "-w" | "--width" | "-g" | "--goal" | "-T" | "--tab-width" => {
                let n = match value.parse::<isize>() {
                    Ok(n) => n,
                    Err(_) => {
                        let _ = stderr.write_all(format!("fmt: invalid number: {}\n", value).as_bytes());
                        return 2;
                    }
                };
                if n < 0 {
                    // man: width can be given as a negative number in the first argument
                    o.width = (-n) as usize;
                } else if a == "-g" || a == "--goal" {
                    o.goal = n as usize;
                } else if a == "-T" || a == "--tab-width" {
                    o.tab_width = n as usize;
                } else {
                    o.width = n as usize;
                }
                i += 1;
            }
            _ => {
                let _ = stderr.write_all(format!("fmt: unknown option: {}\n", a).as_bytes());
                return 2;
            }
        }
        i += 1;
    }
    o.prefix = &prefix_owned;
    o.skip_prefix = &skip_owned;
    if o.goal == 0 {
        o.goal = o.width * 93 / 100;
    }
    if o.goal > o.width {
        let _ = stderr.write_all(b"fmt: goal must be less than or equal to width\n");
        return 2;
    }

    let data: Vec<u8> = if files.is_empty() {
        let mut buf = Vec::new();
        if stdin.read_to_end(&mut buf).is_err() {
            let _ = stderr.write_all(b"fmt: standard input: read error\n");
            return 1;
        }
        buf
    } else {
        let mut buf = Vec::new();
        let mut status = 0;
        for f in &files {
            if f == "-" {
                let mut b = Vec::new();
                if stdin.read_to_end(&mut b).is_err() {
                    status = 1;
                } else {
                    buf.extend_from_slice(&b);
                }
                continue;
            }
            match std::fs::File::open(f) {
                Ok(mut fh) => {
                    let mut b = Vec::new();
                    if fh.read_to_end(&mut b).is_err() {
                        status = 1;
                    } else {
                        buf.extend_from_slice(&b);
                    }
                }
                Err(e) => {
                    let _ = stderr.write_all(format!("fmt: {}: {}\n", f, e).as_bytes());
                    status = 1;
                }
            }
        }
        if status != 0 {
            return status;
        }
        buf
    };

    let text = String::from_utf8_lossy(&data).to_string();
    let mut lines: Vec<String> = text.split('\n').map(|l| l.to_string()).collect();
    if lines.last().map(|l| l.is_empty()).unwrap_or(false) {
        lines.pop();
    }
    let mut i = 0;
    while i < lines.len() {
        if lines[i].is_empty() {
            if stdout.write_all(b"\n").is_err() {
                return 1;
            }
            i += 1;
            continue;
        }
        let mut para = Vec::new();
        while i < lines.len() && !lines[i].is_empty() {
            para.push(lines[i].clone());
            i += 1;
        }
        if emit(&para, stdout, &o) == 1 {
            return 1;
        }
    }
    0
}