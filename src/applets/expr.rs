//! expr：依 man page（docs/man/expr.1.gz）實作。
//!
//! 優先等級（由低到高）：`|`、`&`、比較、`+ -`、`* / %`、atom。
//! 值有三種：數字、字串、null。null 或 0 視為假。
//! 結束狀態：0（表達式非 null 且非 0）、1（null 或 0）、2（語法錯誤）。

use crate::regex;
use crate::In;
use crate::Out;

enum Val {
    Num(i64),
    Str(String),
    Null,
}

impl Val {
    fn truthy(&self) -> bool {
        match self {
            Val::Null => false,
            Val::Num(n) => *n != 0,
            Val::Str(s) => !s.is_empty() && s != "0",
        }
    }

    fn as_i64(&self) -> i64 {
        match self {
            Val::Num(n) => *n,
            Val::Str(s) => s.parse::<i64>().unwrap_or(0),
            Val::Null => 0,
        }
    }

    fn text(&self) -> String {
        match self {
            Val::Num(n) => n.to_string(),
            Val::Str(s) => s.clone(),
            Val::Null => String::new(),
        }
    }
}

struct Parser<'a> {
    toks: &'a [String],
    pos: usize,
    protected: bool,
}

fn level_of(t: &str) -> Option<usize> {
    match t {
        "|" => Some(5),
        "&" => Some(4),
        "<" | "<=" | "=" | "!=" | ">=" | ">" => Some(3),
        "+" | "-" => Some(2),
        "*" | "/" | "%" | ":" => Some(1),
        _ => None,
    }
}

impl<'a> Parser<'a> {
    fn parse(&mut self, level: usize) -> Result<Val, String> {
        if level == 0 {
            return self.parse_atom();
        }
        let mut left = self.parse(level - 1)?;
        if self.protected {
            self.protected = false;
            return Ok(left);
        }
        while self.pos < self.toks.len() && level_of(&self.toks[self.pos]) == Some(level) {
            let op = self.toks[self.pos].clone();
            self.pos += 1;
            let right = self.parse(level - 1)?;
            left = apply(&op, left, right)?;
        }
        Ok(left)
    }

    fn parse_atom(&mut self) -> Result<Val, String> {
        if self.pos >= self.toks.len() {
            return Err("unexpected end of expression".to_string());
        }
        let t = self.toks[self.pos].clone();
        if t == "(" {
            self.pos += 1;
            let v = self.parse(5)?;
            if self.pos >= self.toks.len() || self.toks[self.pos] != ")" {
                return Err("missing closing parenthesis".to_string());
            }
            self.pos += 1;
            return Ok(v);
        }
        self.pos += 1;
        match t.as_str() {
            "+" => {
                if self.pos >= self.toks.len() {
                    return Err("missing token after +".to_string());
                }
                self.protected = true;
                Ok(Val::Str(self.toks[self.pos].clone()))
            }
            "length" => {
                if self.pos >= self.toks.len() {
                    return Err("missing argument to length".to_string());
                }
                let s = self.toks[self.pos].clone();
                self.pos += 1;
                Ok(Val::Num(s.chars().count() as i64))
            }
            "index" => {
                if self.pos + 1 >= self.toks.len() {
                    return Err("missing arguments to index".to_string());
                }
                let s = self.toks[self.pos].clone();
                let chars = self.toks[self.pos + 1].clone();
                self.pos += 2;
                let idx = s.chars().position(|c| chars.contains(c));
                Ok(Val::Num(match idx {
                    Some(i) => (i + 1) as i64,
                    None => 0,
                }))
            }
            "substr" => {
                if self.pos + 2 >= self.toks.len() {
                    return Err("missing arguments to substr".to_string());
                }
                let s = self.toks[self.pos].clone();
                let pos = self.toks[self.pos + 1].parse::<usize>().unwrap_or(0);
                let len = self.toks[self.pos + 2].parse::<usize>().unwrap_or(0);
                self.pos += 3;
                if pos == 0 || len == 0 {
                    return Ok(Val::Num(0));
                }
                let chars: Vec<char> = s.chars().collect();
                let start = pos - 1;
                if start >= chars.len() {
                    return Ok(Val::Null);
                }
                let end = (start + len).min(chars.len());
                Ok(Val::Str(chars[start..end].iter().collect()))
            }
            "match" => {
                if self.pos + 1 >= self.toks.len() {
                    return Err("missing arguments to match".to_string());
                }
                let s = self.toks[self.pos].clone();
                let pat = self.toks[self.pos + 1].clone();
                self.pos += 2;
                Ok(match_value(&s, &pat))
            }
            _ => Ok(Val::Str(t)),
        }
    }
}

fn match_value(s: &str, pat: &str) -> Val {
    let re = regex::compile(pat.as_bytes());
    match regex::match_with_groups(&re, s.as_bytes()) {
        Some((start, end, groups)) => {
            if !groups.is_empty() {
                let g = groups[0];
                if g.1 > g.0 {
                    return Val::Str(String::from_utf8_lossy(&s.as_bytes()[g.0..g.1]).to_string());
                }
                return Val::Null;
            }
            if end > start {
                Val::Num((end - start) as i64)
            } else {
                Val::Null
            }
        }
        None => Val::Num(0),
    }
}

fn apply(op: &str, left: Val, right: Val) -> Result<Val, String> {
    match op {
        "|" => Ok(if left.truthy() { left } else { right }),
        "&" => Ok(if left.truthy() && right.truthy() { left } else { Val::Num(0) }),
        "<" => Ok(cmp(&left, &right, |o| o == std::cmp::Ordering::Less)),
        "<=" => Ok(cmp(&left, &right, |o| o != std::cmp::Ordering::Greater)),
        "=" => Ok(cmp(&left, &right, |o| o == std::cmp::Ordering::Equal)),
        "!=" => Ok(cmp(&left, &right, |o| o != std::cmp::Ordering::Equal)),
        ">=" => Ok(cmp(&left, &right, |o| o != std::cmp::Ordering::Less)),
        ">" => Ok(cmp(&left, &right, |o| o == std::cmp::Ordering::Greater)),
        "+" => Ok(Val::Num(left.as_i64() + right.as_i64())),
        "-" => Ok(Val::Num(left.as_i64() - right.as_i64())),
        "*" => Ok(Val::Num(left.as_i64() * right.as_i64())),
        "/" => {
            let d = right.as_i64();
            if d == 0 {
                return Err("division by zero".to_string());
            }
            Ok(Val::Num(left.as_i64() / d))
        }
        ":" => Ok(match_value(&left.text(), &right.text())),
        "%" => {
            let d = right.as_i64();
            if d == 0 {
                return Err("division by zero".to_string());
            }
            Ok(Val::Num(left.as_i64() % d))
        }
        _ => Err(format!("unknown operator {op}")),
    }
}

fn cmp(left: &Val, right: &Val, pred: fn(std::cmp::Ordering) -> bool) -> Val {
    let both_numeric = left.text().parse::<i64>().is_ok() && right.text().parse::<i64>().is_ok();
    let ord = if both_numeric {
        left.as_i64().cmp(&right.as_i64())
    } else {
        left.text().cmp(&right.text())
    };
    Val::Num(if pred(ord) { 1 } else { 0 })
}

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    if args.is_empty() {
        println!("expr: missing expression");
        return 2;
    }
    if args[0] == "--help" || args[0] == "-h" {
        if stdout.write_all(b"Usage: expr [EXPRESSION]\n").is_err() {
            return 3;
        }
        return 0;
    }
    if args[0] == "--version" || args[0] == "-V" {
        if stdout.write_all(b"expr (uutils coreutils) 0.10.0\n").is_err() {
            return 3;
        }
        return 0;
    }
    let mut p = Parser { toks: args, pos: 0, protected: false };
    match p.parse(5) {
        Ok(v) => {
            let s = v.text();
            if stdout.write_all(s.as_bytes()).is_err() || stdout.write_all(&[b'\n']).is_err() {
                return 3;
            }
            if v.truthy() {
                0
            } else {
                1
            }
        }
        Err(e) => {
            if stderr.write_all(format!("expr: {e}\n").as_bytes()).is_err() {
                return 3;
            }
            2
        }
    }
}
