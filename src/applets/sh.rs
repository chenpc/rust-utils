//! sh — 極簡 POSIX 風 shell（取代 guest 裡的 busybox ash）。
//!
//! 支援：執行外部命令（PATH 查找，走 fork/exec）、管線 `|`、`&&`/`||`、`;`、
//! 單/雙引號、`$VAR`/`${VAR}`/`$?` 展開、變數指派與 `export`、
//! `if/then/elif/else/fi`、`for..in..do..done`、`>`、`>>`、`<`、`2>`、
//! 內建：`cd pwd echo export unset [ true false exit`。
//! 互動模式（stdin 為 tty）有行編輯、歷史（↑/↓）與 **Tab 自動補全**
//! （命令名補全含所有 applet symlink 與內建；參數補檔名）。
//!
//! 不支援（極簡取捨）：背景執行 `&`、`case`、`while`、function、subshell `()`、
//! 命令取代 `$()`、glob、`2>&1`。變數指派一律等同 export（guest 內無分程需求）。
//! 行為依據：POSIX sh 子集合 + 專案 dev log 記錄的取捨。

use std::collections::HashMap;
use std::io::{Read, Write};
use std::process::{Command, Stdio};

use crate::sys;

type In<'a> = &'a mut dyn Read;
type Out<'a> = &'a mut dyn Write;

pub fn run(args: &[String], stdin: In<'_>, stdout: Out<'_>, stderr: Out<'_>) -> i32 {
    let mut it = args.iter();
    let mut cmd_string: Option<String> = None;
    let mut script: Option<String> = None;
    let mut forced_interactive = false;
    let mut forced_script = false;
    while let Some(a) = it.next() {
        match a.as_str() {
            "-c" => cmd_string = it.next().cloned(),
            "-i" => forced_interactive = true,
            "-n" => forced_script = true,
            "-h" | "--help" => {
                let _ = writeln!(
                    stdout,
                    "usage: sh [-c command] [script] [args...]\n\
                     minimal shell: pipelines, if/for, quoting, $VAR, builtins,\n\
                     interactive mode with history and tab completion"
                );
                return 0;
            }
            _ => {
                if script.is_none() {
                    script = Some(a.clone());
                }
            }
        }
    }

    if let Some(c) = cmd_string {
        let mut sh = new_interp(stdin, stdout, stderr);
        return sh.run_text(&c);
    }
    if let Some(path) = script {
        return run_script(&path, stdin, stdout, stderr);
    }
    let mut sh = new_interp(stdin, stdout, stderr);
    if !forced_script && (forced_interactive || sys::stdin_is_tty()) {
        return sh.interactive();
    }
    let mut text = String::new();
    let _ = sh.inp.read_to_string(&mut text);
    sh.run_text(&text)
}

fn new_interp<'a>(stdin: In<'a>, stdout: Out<'a>, stderr: Out<'a>) -> Interp<'a> {
    Interp {
        inp: Box::new(stdin),
        out: Box::new(stdout),
        err: Box::new(stderr),
        env: HashMap::new(),
        last: 0,
        depth: 0,
    }
}

/// 跑腳本檔（init 執行 payload 用；stdin 由呼叫端決定，可給 /dev/null）。
pub fn run_script(path: &str, stdin: In<'_>, stdout: Out<'_>, stderr: Out<'_>) -> i32 {
    let mut sh = new_interp(stdin, stdout, stderr);
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            let _ = writeln!(sh.err, "sh: cannot open {path}: {e}");
            return 2;
        }
    };
    sh.run_text(&text)
}

// ---------------------------------------------------------------------------
// tokenizer
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Seg {
    Lit(String),
    Var(String), // VAR、${VAR}、?（$?）
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Word(Vec<Seg>),
    Op(&'static str), // ; | && || < > >> 2>
    Newline,
}

fn is_word_char(c: char) -> bool {
    !matches!(
        c,
        ' ' | '\t' | '\n' | ';' | '|' | '&' | '<' | '>' | '(' | ')' | '"' | '\'' | '$'
    )
}

fn scan_var(chars: &[char], i: &mut usize) -> Option<Seg> {
    // chars[*i] == '$'；回傳Var段並推進i，無法展開時回None
    let mut j = *i + 1;
    if chars.get(j) == Some(&'{') {
        j += 1;
        let start = j;
        while j < chars.len() && chars[j] != '}' {
            j += 1;
        }
        if j >= chars.len() {
            return None;
        }
        let name: String = chars[start..j].iter().collect();
        *i = j + 1;
        return Some(Seg::Var(name));
    }
    if chars.get(j) == Some(&'?') {
        *i = j + 1;
        return Some(Seg::Var("?".to_string()));
    }
    if j < chars.len() && (chars[j] == '_' || chars[j].is_alphanumeric()) {
        let start = j;
        while j < chars.len() && (chars[j] == '_' || chars[j].is_alphanumeric()) {
            j += 1;
        }
        let name: String = chars[start..j].iter().collect();
        *i = j;
        return Some(Seg::Var(name));
    }
    None
}

fn tokenize(src: &str) -> Result<Vec<Tok>, String> {
    let mut toks = Vec::new();
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' => i += 1,
            '\n' => {
                toks.push(Tok::Newline);
                i += 1;
            }
            '#' => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            ';' => {
                toks.push(Tok::Op(";"));
                i += 1;
            }
            '|' if chars.get(i + 1) == Some(&'|') => {
                toks.push(Tok::Op("||"));
                i += 2;
            }
            '&' if chars.get(i + 1) == Some(&'&') => {
                toks.push(Tok::Op("&&"));
                i += 2;
            }
            '|' => {
                toks.push(Tok::Op("|"));
                i += 1;
            }
            '>' if chars.get(i + 1) == Some(&'>') => {
                toks.push(Tok::Op(">>"));
                i += 2;
            }
            '>' => {
                toks.push(Tok::Op(">"));
                i += 1;
            }
            '<' => {
                toks.push(Tok::Op("<"));
                i += 1;
            }
            '2' if chars.get(i + 1) == Some(&'>')
                && (i == 0 || !is_word_char(chars[i - 1])) =>
            {
                toks.push(Tok::Op("2>"));
                i += 2;
            }
            c if c == '"' || c == '\'' || c == '$' || is_word_char(c) => {
                let mut w: Vec<Seg> = Vec::new();
                let mut cur = String::new();
                while i < chars.len() {
                    let c = chars[i];
                    if !is_word_char(c) && c != '"' && c != '\'' && c != '$' {
                        break;
                    }
                    match c {
                        '"' => {
                            if !cur.is_empty() {
                                w.push(Seg::Lit(std::mem::take(&mut cur)));
                            }
                            i += 1;
                            while i < chars.len() && chars[i] != '"' {
                                let c = chars[i];
                                if c == '\\'
                                    && matches!(
                                        chars.get(i + 1),
                                        Some('"') | Some('\\') | Some('$')
                                    )
                                {
                                    cur.push(chars[i + 1]);
                                    i += 2;
                                    continue;
                                }
                                if c == '$' {
                                    if let Some(seg) = scan_var(&chars, &mut i) {
                                        if !cur.is_empty() {
                                            w.push(Seg::Lit(std::mem::take(&mut cur)));
                                        }
                                        w.push(seg);
                                    } else {
                                        cur.push('$');
                                        i += 1;
                                    }
                                    continue;
                                }
                                cur.push(c);
                                i += 1;
                            }
                            if i >= chars.len() {
                                return Err("unterminated \"".to_string());
                            }
                            i += 1;
                        }
                        '\'' => {
                            if !cur.is_empty() {
                                w.push(Seg::Lit(std::mem::take(&mut cur)));
                            }
                            i += 1;
                            let start = i;
                            while i < chars.len() && chars[i] != '\'' {
                                i += 1;
                            }
                            if i >= chars.len() {
                                return Err("unterminated '".to_string());
                            }
                            w.push(Seg::Lit(chars[start..i].iter().collect()));
                            i += 1;
                        }
                        '$' => {
                            i += 1;
                            if chars.get(i) == Some(&'{') {
                                i += 1;
                                let start = i;
                                while i < chars.len() && chars[i] != '}' {
                                    i += 1;
                                }
                                if i >= chars.len() {
                                    return Err("unterminated ${".to_string());
                                }
                                if !cur.is_empty() {
                                    w.push(Seg::Lit(std::mem::take(&mut cur)));
                                }
                                w.push(Seg::Var(chars[start..i].iter().collect()));
                                i += 1;
                            } else if chars.get(i) == Some(&'?') {
                                if !cur.is_empty() {
                                    w.push(Seg::Lit(std::mem::take(&mut cur)));
                                }
                                w.push(Seg::Var("?".to_string()));
                                i += 1;
                            } else if i < chars.len()
                                && (chars[i] == '_' || chars[i].is_alphanumeric())
                            {
                                let start = i;
                                while i < chars.len()
                                    && (chars[i] == '_'
                                        || chars[i].is_alphanumeric())
                                {
                                    i += 1;
                                }
                                if !cur.is_empty() {
                                    w.push(Seg::Lit(std::mem::take(&mut cur)));
                                }
                                w.push(Seg::Var(chars[start..i].iter().collect()));
                            } else {
                                cur.push('$');
                            }
                        }
                        '\\' if chars.get(i + 1).is_some() => {
                            i += 1;
                            cur.push(chars[i]);
                            i += 1;
                        }
                        _ => {
                            cur.push(c);
                            i += 1;
                        }
                    }
                }
                if !cur.is_empty() {
                    w.push(Seg::Lit(cur));
                }
                toks.push(Tok::Word(w));
            }
            other => {
                return Err(format!("unexpected character '{other}'"));
            }
        }
    }
    Ok(toks)
}

// ---------------------------------------------------------------------------
// parser
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct Redir {
    op: &'static str, // > >> < 2>
    file: Vec<Seg>,
}

#[derive(Debug)]
enum Cmd {
    External {
        argv: Vec<Vec<Seg>>,
        redirs: Vec<Redir>,
    },
    Assign {
        name: String,
        value: Vec<Seg>,
    },
    If {
        conds: Vec<(Vec<Stmt>, Vec<Stmt>)>,
        else_br: Option<Vec<Stmt>>,
    },
    For {
        var: String,
        list: Vec<Vec<Seg>>,
        body: Vec<Stmt>,
    },
}

#[derive(Debug)]
struct Pipeline {
    parts: Vec<Cmd>,
    redirs: Vec<Redir>,
}

#[derive(Debug)]
enum Stmt {
    Pipe(Pipeline),
    AndOr(Box<Stmt>, &'static str, Box<Stmt>),
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

const CONTROL: &[&str] = &["if", "then", "elif", "else", "fi", "for", "in", "do", "done"];

impl Parser {
    fn peek_word(&self) -> Option<&str> {
        match self.toks.get(self.pos)? {
            Tok::Word(segs) => {
                if segs.len() == 1 {
                    if let Seg::Lit(s) = &segs[0] {
                        return Some(s.as_str());
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn skip_newlines(&mut self) {
        while matches!(self.toks.get(self.pos), Some(Tok::Newline)) {
            self.pos += 1;
        }
    }

    fn at_list_end(&self) -> bool {
        match self.toks.get(self.pos) {
            None | Some(Tok::Op(";")) => true,
            Some(Tok::Word(_)) => matches!(self.peek_word(), Some(k) if CONTROL.contains(&k)),
            _ => false,
        }
    }

    /// 解析到 `done`/`fi`/`elif`/`else`/`;`/換行結束的語句清單。
    fn parse_list(&mut self, stop_kw: &[&str]) -> Result<Vec<Stmt>, String> {
        let mut out = Vec::new();
        self.skip_newlines();
        loop {
            if self.pos >= self.toks.len() {
                break;
            }
            if matches!(self.peek_word(), Some(k) if stop_kw.contains(&k)) {
                break;
            }
            let s = self.parse_and_or()?;
            out.push(s);
            match self.toks.get(self.pos) {
                Some(Tok::Op(";")) | Some(Tok::Newline) => self.skip_newlines_nosemi(),
                _ => break,
            }
            self.skip_newlines();
        }
        Ok(out)
    }

    fn skip_newlines_nosemi(&mut self) {
        if matches!(self.toks.get(self.pos), Some(Tok::Op(";"))) {
            self.pos += 1;
        }
        self.skip_newlines();
    }

    fn parse_and_or(&mut self) -> Result<Stmt, String> {
        let mut lhs = Stmt::Pipe(self.parse_pipeline()?);
        loop {
            let op = match self.toks.get(self.pos) {
                Some(Tok::Op("&&")) | Some(Tok::Op("||")) => {
                    if let Tok::Op(o) = self.toks[self.pos] {
                        o
                    } else {
                        unreachable!()
                    }
                }
                _ => break,
            };
            self.pos += 1;
            self.skip_newlines();
            // 左結合：rhs 只取單一 pipeline，避免 a && b || c 變成 a && (b || c)
            let rhs = Box::new(Stmt::Pipe(self.parse_pipeline()?));
            lhs = Stmt::AndOr(Box::new(lhs), op, rhs);
        }
        Ok(lhs)
    }

    fn parse_pipeline(&mut self) -> Result<Pipeline, String> {
        let mut parts = vec![self.parse_command()?];
        let mut redirs = Vec::new();
        // 命令本體的 redir 記在最後一個 part 之外（套用到整條管線的最後輸出）
        while matches!(self.toks.get(self.pos), Some(Tok::Op("|"))) {
            self.pos += 1;
            self.skip_newlines();
            parts.push(self.parse_command()?);
        }
        while let Some(Tok::Op(op @ (">" | ">>" | "<" | "2>"))) = self.toks.get(self.pos) {
            let op = *op;
            self.pos += 1;
            let file = match self.toks.get(self.pos) {
                Some(Tok::Word(w)) => {
                    let w = w.clone();
                    self.pos += 1;
                    w
                }
                _ => return Err(format!("missing file after {op}")),
            };
            redirs.push(Redir { op, file });
        }
        Ok(Pipeline { parts, redirs })
    }

    fn parse_command(&mut self) -> Result<Cmd, String> {
        match self.peek_word() {
            Some("if") => self.parse_if(),
            Some("for") => self.parse_for(),
            _ => self.parse_simple(),
        }
    }

    fn parse_simple(&mut self) -> Result<Cmd, String> {
        let mut argv: Vec<Vec<Seg>> = Vec::new();
        let mut redirs: Vec<Redir> = Vec::new();
        while let Some(t) = self.toks.get(self.pos) {
            match t {
                Tok::Word(_) => break,
                Tok::Op(op @ (">" | ">>" | "<" | "2>")) => {
                    let op = *op;
                    self.pos += 1;
                    let file = match self.toks.get(self.pos) {
                        Some(Tok::Word(w)) => {
                            let w = w.clone();
                            self.pos += 1;
                            w
                        }
                        _ => return Err(format!("missing file after {op}")),
                    };
                    redirs.push(Redir { op, file });
                }
                _ => break,
            }
        }
        // 保留字只在 argv 空（命令位）時終止；`echo done` 的 done 是一般 arg
        while self.pos < self.toks.len()
            && !(self.at_list_end() && argv.is_empty())
            && !matches!(self.toks.get(self.pos), Some(Tok::Op("|" | "&&" | "||")))
        {
            match &self.toks[self.pos] {
                Tok::Word(segs) => {
                    let segs = segs.clone();
                    self.pos += 1;
                    argv.push(segs);
                }
                Tok::Op(op @ (">" | ">>" | "<" | "2>")) => {
                    let op = *op;
                    self.pos += 1;
                    let file = match self.toks.get(self.pos) {
                        Some(Tok::Word(w)) => {
                            let w = w.clone();
                            self.pos += 1;
                            w
                        }
                        _ => return Err(format!("missing file after {op}")),
                    };
                    redirs.push(Redir { op, file });
                }
                _ => break,
            }
        }
        if argv.is_empty() {
            if redirs.is_empty() {
                return Err("empty command".to_string());
            }
            return Ok(Cmd::External { argv: vec![], redirs });
        }
        // 指派：第一個 word 是裸的 NAME=...
        if let Some(first) = argv.first() {
            if let Some((name, lit)) = assign_name(first) {
                let mut v: Vec<Seg> = Vec::new();
                if let Some(rest) = lit {
                    v.push(Seg::Lit(rest));
                }
                for w in &argv[1..] {
                    if !v.is_empty() {
                        v.push(Seg::Lit(" ".to_string()));
                    }
                    v.extend_from_slice(w);
                }
                let _ = redirs; // 指派不處理 redir（極簡）
                return Ok(Cmd::Assign { name, value: v });
            }
        }
        Ok(Cmd::External { argv, redirs })
    }

    fn parse_if(&mut self) -> Result<Cmd, String> {
        self.pos += 1; // if
        let mut conds = Vec::new();
        loop {
            let cond = self.parse_list(&["then"])?;
            if self.peek_word() != Some("then") {
                return Err("expected then".to_string());
            }
            self.pos += 1;
            let body = self.parse_list(&["elif", "else", "fi"])?;
            conds.push((cond, body));
            match self.peek_word() {
                Some("elif") => {
                    self.pos += 1;
                }
                Some("else") => {
                    self.pos += 1;
                    let else_br = self.parse_list(&["fi"])?;
                    if self.peek_word() != Some("fi") {
                        return Err("expected fi".to_string());
                    }
                    self.pos += 1;
                    return Ok(Cmd::If { conds, else_br: Some(else_br) });
                }
                Some("fi") => {
                    self.pos += 1;
                    return Ok(Cmd::If { conds, else_br: None });
                }
                _ => return Err("expected elif/else/fi".to_string()),
            }
        }
    }

    fn parse_for(&mut self) -> Result<Cmd, String> {
        self.pos += 1; // for
        let var = match self.peek_word() {
            Some(v) if !CONTROL.contains(&v) => {
                let v = v.to_string();
                self.pos += 1;
                v
            }
            _ => return Err("expected variable after for".to_string()),
        };
        let mut list = Vec::new();
        if self.peek_word() == Some("in") {
            self.pos += 1;
            while matches!(self.toks.get(self.pos), Some(Tok::Word(_)))
                && self.peek_word() != Some("do")
                && self.peek_word() != Some("done")
            {
                if let Tok::Word(w) = &self.toks[self.pos] {
                    list.push(w.clone());
                    self.pos += 1;
                }
            }
            self.skip_newlines();
        }
        // `for x in a b; do` — 清單後可能還有 ; 或換行
        while matches!(self.toks.get(self.pos), Some(Tok::Op(";"))) {
            self.pos += 1;
            self.skip_newlines();
        }
        if self.peek_word() != Some("do") {
            return Err("expected do".to_string());
        }
        self.pos += 1;
        let body = self.parse_list(&["done"])?;
        if self.peek_word() != Some("done") {
            return Err("expected done".to_string());
        }
        self.pos += 1;
        Ok(Cmd::For { var, list, body })
    }
}

/// 若 word 是裸的 `NAME=` 開頭（第一個 segment 是 Lit 且含 `=`），回 (NAME, 剩餘字面)。
fn assign_name(segs: &[Seg]) -> Option<(String, Option<String>)> {
    if let Some(Seg::Lit(l)) = segs.first() {
        if let Some(eq) = l.find('=') {
            let name = &l[..eq];
            let ok = !name.is_empty()
                && (name.starts_with(|c: char| c.is_alphabetic() || c == '_'))
                && name.chars().all(|c| c == '_' || c.is_alphanumeric());
            if ok {
                let rest = &l[eq + 1..];
                return Some((name.to_string(), if rest.is_empty() { None } else { Some(rest.to_string()) }));
            }
        }
    }
    None
}

fn parse(src: &str) -> Result<Vec<Stmt>, String> {
    let toks = tokenize(src)?;
    let mut p = Parser { toks, pos: 0 };
    let list = p.parse_list(&[])?;
    p.skip_newlines();
    if p.pos < p.toks.len() {
        return Err(format!("unconsumed input at token {}", p.pos));
    }
    Ok(list)
}

// ---------------------------------------------------------------------------
// interpreter
// ---------------------------------------------------------------------------

struct Interp<'a> {
    inp: Box<dyn Read + 'a>,
    out: Box<dyn Write + 'a>,
    err: Box<dyn Write + 'a>,
    env: HashMap<String, String>,
    last: i32,
    depth: u32,
}

fn builtin_names() -> Vec<&'static str> {
    vec!["cd", "pwd", "echo", "export", "unset", "[", "test", "true", "false", "exit"]
}

impl<'a> Interp<'a> {
    fn expand_word(&self, segs: &[Seg]) -> String {
        let mut s = String::new();
        for seg in segs {
            match seg {
                Seg::Lit(l) => s.push_str(l),
                Seg::Var(name) => {
                    if name == "?" {
                        s.push_str(&self.last.to_string());
                    } else {
                        if let Some(v) = self.env.get(name) {
                            s.push_str(v);
                        } else if let Ok(v) = std::env::var(name) {
                            s.push_str(&v);
                        }
                    }
                }
            }
        }
        s
    }

    fn run_text(&mut self, text: &str) -> i32 {
        match parse(text) {
            Ok(list) => match self.run_list(&list) {
                Ok(code) | Err(code) => code,
            },
            Err(e) => {
                let _ = writeln!(self.err, "sh: syntax error: {e}");
                2
            }
        }
    }

    fn run_list(&mut self, list: &[Stmt]) -> Result<i32, i32> {
        let mut code = 0;
        for s in list {
            code = self.run_stmt(s)?;
            self.last = code; // 語句間 $? 要即時更新
        }
        Ok(code)
    }

    fn run_stmt(&mut self, s: &Stmt) -> Result<i32, i32> {
        match s {
            Stmt::Pipe(p) => self.run_pipeline(p),
            Stmt::AndOr(l, op, r) => {
                let lc = self.run_stmt(l)?;
                if (*op == "&&" && lc != 0) || (*op == "||" && lc == 0) {
                    return Ok(lc);
                }
                self.run_stmt(r)
            }
        }
    }

    fn run_pipeline(&mut self, p: &Pipeline) -> Result<i32, i32> {
        if p.parts.len() == 1 {
            return self.run_cmd(&p.parts[0], &p.redirs);
        }
        // 管線：外部命令用 OS pipe 串接；內建用 std::os::unix::pipe 模擬（test guest 的
        // /bin 沒有 applet symlink，純外部 echo 會 spawn 失敗）。
        let _ = &p.redirs;
        enum Prev {
            Child(std::process::ChildStdout),
            Pipe(std::os::fd::OwnedFd),
            Null,
        }
        let mut prev: Option<Prev> = None;
        let mut last_status = 0;
        let n = p.parts.len();
        for (idx, part) in p.parts.iter().enumerate() {
            let (argv, redirs) = match part {
                Cmd::External { argv, redirs } if !argv.is_empty() => (argv, redirs),
                Cmd::External { .. } => {
                    last_status = 0;
                    continue;
                }
                Cmd::Assign { .. } | Cmd::If { .. } | Cmd::For { .. } => {
                    let _ = writeln!(self.err, "sh: pipeline element not supported");
                    return Ok(2);
                }
            };
            let exe = self.expand_word(&argv[0]);
            if builtin_names().contains(&exe.as_str()) {
                // 內建：尾段直接跑（輸出去 stdout）；非尾段寫進 pipe 給下一個元素
                if idx == n - 1 {
                    let _ = prev.take();
                    // 套用管線級重導向：暫時換掉 interp 串流
                    let mut saved_out: Option<Box<dyn std::io::Write>> = None;
                    let mut saved_err: Option<Box<dyn std::io::Write>> = None;
                    for r in &p.redirs {
                        let f = self.open_out(r)?;
                        match r.op {
                            ">" | ">>" => {
                                if saved_out.is_none() {
                                    saved_out =
                                        Some(std::mem::replace(&mut self.out, Box::new(f)));
                                }
                            }
                            "2>" => {
                                if saved_err.is_none() {
                                    saved_err =
                                        Some(std::mem::replace(&mut self.err, Box::new(f)));
                                }
                            }
                            _ => {}
                        }
                    }
                    last_status = match self.run_cmd(part, &[]) {
                        Ok(c) => c,
                        Err(c) => c,
                    };
                    if let Some(old) = saved_out {
                        self.out = old;
                    }
                    if let Some(old) = saved_err {
                        self.err = old;
                    }
                    continue;
                }
                let (rx, tx) = match crate::sys::anon_pipe() {
                    Ok(v) => v,
                    Err(_) => return Ok(2),
                };
                let saved = std::mem::replace(&mut self.out, Box::new(std::fs::File::from(tx)));
                let code = match self.run_cmd(part, &[]) {
                    Ok(c) => c,
                    Err(c) => c,
                };
                self.out = saved; // drop sender → 下游 EOF
                last_status = code;
                prev = Some(Prev::Pipe(rx));
                continue;
            }
            let mut cmd = Command::new(&exe);
            for a in &argv[1..] {
                cmd.arg(self.expand_word(a));
            }
            let mut redir_out = false;
            for r in redirs {
                let _ = self.apply_redir(&mut cmd, r)?;
                if matches!(r.op, ">" | ">>") {
                    redir_out = true;
                }
            }
            if idx == n - 1 {
                for r in &p.redirs {
                    let _ = self.apply_redir(&mut cmd, r)?;
                    match r.op {
                        ">" | ">>" => redir_out = true,
                        _ => {}
                    }
                }
            }
            match prev.take() {
                Some(Prev::Child(o)) => cmd.stdin(Stdio::from(o)),
                Some(Prev::Pipe(r)) => cmd.stdin(Stdio::from(r)),
                // 前一元素失敗：給 null，不讓下游讀 console 卡死
                Some(Prev::Null) => cmd.stdin(Stdio::null()),
                None => cmd.stdin(Stdio::inherit()),
            };
            if idx == n - 1 {
                if !redir_out {
                    cmd.stdout(Stdio::inherit());
                }
            } else {
                cmd.stdout(Stdio::piped());
            }
            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    let _ = writeln!(self.err, "sh: {exe}: {e}");
                    last_status = 127;
                    prev = Some(Prev::Null);
                    continue;
                }
            };
            prev = child.stdout.take().map(Prev::Child);
            last_status = child
                .wait()
                .map(|s| s.code().unwrap_or(1))
                .unwrap_or(1);
        }
        Ok(last_status)
    }

    fn open_out(&mut self, r: &Redir) -> Result<std::fs::File, i32> {
        let path = self.expand_word(&r.file);
        let opened = match r.op {
            ">" | "2>" => std::fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&path),
            ">>" => std::fs::OpenOptions::new().create(true).append(true).open(&path),
            _ => std::fs::File::open(&path),
        };
        match opened {
            Ok(f) => Ok(f),
            Err(e) => {
                let _ = writeln!(self.err, "sh: {path}: {e}");
                Err(1)
            }
        }
    }

    fn apply_redir(&mut self, cmd: &mut Command, r: &Redir) -> Result<(), i32> {
        let f = self.open_out(r)?;
        match r.op {
            ">" | ">>" => cmd.stdout(Stdio::from(f)),
            "<" => cmd.stdin(Stdio::from(f)),
            "2>" => cmd.stderr(Stdio::from(f)),
            _ => cmd.stderr(Stdio::inherit()),
        };
        Ok(())
    }

    fn run_cmd(&mut self, c: &Cmd, outer: &[Redir]) -> Result<i32, i32> {
        self.depth += 1;
        if self.depth > 64 {
            self.depth -= 1;
            let _ = writeln!(self.err, "sh: nesting too deep");
            return Ok(2);
        }
        let r = match c {
            Cmd::Assign { name, value } => {
                let v = self.expand_word(value);
                self.env.insert(name.clone(), v.clone());
                std::env::set_var(name, v);
                Ok(0)
            }
            Cmd::If { conds, else_br } => {
                let mut chosen: Option<&Vec<Stmt>> = None;
                for (cond, body) in conds {
                    let code = self.run_list(cond)?;
                    if code == 0 {
                        chosen = Some(body);
                        break;
                    }
                }
                let body = chosen.or(else_br.as_ref());
                match body {
                    Some(b) => self.run_list(b),
                    None => Ok(0),
                }
            }
            Cmd::For { var, list, body } => {
                let items: Vec<String> = list.iter().map(|w| self.expand_word(w)).collect();
                let mut code = 0;
                for item in items {
                    self.env.insert(var.clone(), item.clone());
                    std::env::set_var(var, &item);
                    code = self.run_list(body)?;
                }
                Ok(code)
            }
            Cmd::External { argv, redirs } => self.run_external(argv, redirs, outer),
        };
        self.depth -= 1;
        r
    }

    fn run_external(
        &mut self,
        argv: &[Vec<Seg>],
        redirs: &[Redir],
        outer: &[Redir],
    ) -> Result<i32, i32> {
        if argv.is_empty() {
            return Ok(0);
        }
        let name = self.expand_word(&argv[0]);
        let words: Vec<String> = argv[1..].iter().map(|w| self.expand_word(w)).collect();
        if builtin_names().contains(&name.as_str()) {
            return self.run_builtin(&name, &words, redirs, outer);
        }
        let mut cmd = Command::new(&name);
        cmd.args(&words);
        for r in redirs.iter().chain(outer.iter()) {
            self.apply_redir(&mut cmd, r)?;
        }
        match cmd.spawn().and_then(|mut c| c.wait()) {
            Ok(s) => Ok(s.code().unwrap_or(1)),
            Err(e) => {
                // spawn 失敗的錯誤訊息：若本命令有 2> 重導向 → 寫進目標檔（POSIX）
                let msg = format!("sh: {name}: {e}");
                let mut wrote = false;
                for r in redirs.iter().chain(outer.iter()) {
                    if r.op == "2>" {
                        if let Ok(mut f) = self.open_out(r) {
                            let _ = writeln!(f, "{msg}");
                            wrote = true;
                            break;
                        }
                    }
                }
                if !wrote {
                    let _ = writeln!(self.err, "{msg}");
                }
                Ok(127)
            }
        }
    }

    fn run_builtin(
        &mut self,
        name: &str,
        args: &[String],
        redirs: &[Redir],
        outer: &[Redir],
    ) -> Result<i32, i32> {
        // 內建的輸出重導：開檔後暫時替換 self.out/err/inp，結束後還原。
        let mut saved_out: Option<Box<dyn Write>> = None;
        let mut saved_err: Option<Box<dyn Write>> = None;
        let mut saved_inp: Option<Box<dyn Read>> = None;
        for r in redirs.iter().chain(outer.iter()) {
            match r.op {
                ">" | ">>" => {
                    let f = self.open_out(r)?;
                    if saved_out.is_none() {
                        saved_out = Some(std::mem::replace(&mut self.out, Box::new(std::io::sink())));
                    }
                    self.out = Box::new(f);
                }
                "2>" => {
                    let f = self.open_out(r)?;
                    if saved_err.is_none() {
                        saved_err = Some(std::mem::replace(&mut self.err, Box::new(std::io::sink())));
                    }
                    self.err = Box::new(f);
                }
                "<" => {
                    let f = self.open_out(r)?;
                    if saved_inp.is_none() {
                        saved_inp = Some(std::mem::replace(&mut self.inp, Box::new(std::io::empty())));
                    }
                    self.inp = Box::new(f);
                }
                _ => {}
            }
        }
        let rc = self.builtin(name, args);
        let _ = self.out.flush();
        let _ = self.err.flush();
        if let Some(o) = saved_out {
            self.out = o;
        }
        if let Some(e) = saved_err {
            self.err = e;
        }
        if let Some(i) = saved_inp {
            self.inp = i;
        }
        rc
    }

    fn builtin(&mut self, name: &str, args: &[String]) -> Result<i32, i32> {
        match name {
            "cd" => {
                let dir = match args.first() {
                    None => std::env::var("HOME").unwrap_or_else(|_| "/".to_string()),
                    Some(s) if s == "~" || s.is_empty() => {
                        std::env::var("HOME").unwrap_or_else(|_| "/".to_string())
                    }
                    Some(s) => s.clone(),
                };
                match std::env::set_current_dir(&dir) {
                    Ok(()) => {
                        std::env::set_var("PWD", std::env::current_dir().unwrap_or_default());
                        Ok(0)
                    }
                    Err(e) => {
                        let _ = writeln!(self.err, "sh: cd: {dir}: {e}");
                        Ok(1)
                    }
                }
            }
            "pwd" => {
                let d = std::env::current_dir().unwrap_or_default();
                let _ = writeln!(self.out, "{}", d.display());
                Ok(0)
            }
            "echo" => {
                let (nl, rest): (bool, &[String]) =
                    if args.first().map(String::as_str) == Some("-n") { (false, &args[1..]) } else { (true, args) };
                let line = rest.join(" ");
                if nl {
                    let _ = writeln!(self.out, "{line}");
                } else {
                    let _ = write!(self.out, "{line}");
                }
                let _ = self.out.flush();
                Ok(0)
            }
            "export" => {
                for a in args {
                    if let Some((k, _)) = a.split_once('=') {
                        std::env::set_var(k, a[k.len() + 1..].to_string());
                    }
                }
                Ok(0)
            }
            "unset" => {
                for a in args {
                    self.env.remove(a);
                    std::env::remove_var(a);
                }
                Ok(0)
            }
            "true" => Ok(0),
            "false" => Ok(1),
            "exit" => {
                let code = args
                    .first()
                    .and_then(|s| s.parse::<i32>().ok())
                    .unwrap_or(self.last);
                Err(code)
            }
            "[" | "test" => {
                let inner: &[String] = if name == "[" {
                    match args.last().map(String::as_str) {
                        Some("]") => &args[..args.len() - 1],
                        _ => {
                            let _ = writeln!(self.err, "sh: {name}: missing ]");
                            return Ok(2);
                        }
                    }
                } else {
                    args
                };
                let strs: Vec<&str> = inner.iter().map(String::as_str).collect();
                Ok((!test_expr(&strs)) as i32) // shell 慣例：真→0、假→1
            }
            _ => Ok(0),
        }
    }
}

/// -o / -a / 括號（頂層，先切 -o 再切 -a）。
fn test_expr(a: &[&str]) -> bool {
    if a.is_empty() {
        return false;
    }
    // 外層括號若涵蓋整個表達式 → 剝掉再遞迴
    if a[0] == "(" {
        let mut depth = 0usize;
        for (i, x) in a.iter().enumerate() {
            match *x {
                "(" => depth += 1,
                ")" => {
                    depth -= 1;
                    if depth == 0 {
                        if i + 1 == a.len() {
                            return test_expr(&a[1..i]);
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
    }
    // 深度 0 才切 -o / -a（括號內的先由遞迴處理）
    if let Some(p) = find_op_depth0(a, "-o") {
        return test_expr(&a[..p]) || test_expr(&a[p + 1..]);
    }
    if let Some(p) = find_op_depth0(a, "-a") {
        return test_expr(&a[..p]) && test_expr(&a[p + 1..]);
    }
    eval_atom(a)
}

fn find_op_depth0(a: &[&str], op: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, x) in a.iter().enumerate() {
        match *x {
            "(" => depth += 1,
            ")" => depth = depth.saturating_sub(1),
            _ => {
                if depth == 0 && *x == op {
                    return Some(i);
                }
            }
        }
    }
    None
}

fn eval_atom(a: &[&str]) -> bool {
    match a {
        [] => false,
        [x] => !x.is_empty(),
        [x, op, y] => match *op {
            "=" => x == y,
            "!=" => x != y,
            _ => match (
                *op,
                std::path::Path::new(x).symlink_metadata(),
                std::path::Path::new(y).symlink_metadata(),
            ) {
                ("-nt", Ok(m1), Ok(m2)) => newer(&m1, &m2),
                ("-ot", Ok(m1), Ok(m2)) => newer(&m2, &m1),
                _ => false,
            },
        },
        [op, x] => match *op {
            "-e" => std::path::Path::new(x).symlink_metadata().is_ok(),
            "-f" => std::path::Path::new(x)
                .symlink_metadata()
                .map(|m| m.is_file())
                .unwrap_or(false),
            "-d" => std::path::Path::new(x)
                .symlink_metadata()
                .map(|m| m.is_dir())
                .unwrap_or(false),
            "-x" => {
                use std::os::unix::fs::PermissionsExt;
                match std::path::Path::new(x).symlink_metadata() {
                    Ok(m) => m.permissions().mode() & 0o111 != 0,
                    Err(_) => false,
                }
            }
            "-n" => !x.is_empty(),
            "-z" => x.is_empty(),
            "-s" => std::path::Path::new(x)
                .metadata()
                .map(|m| m.len() > 0)
                .unwrap_or(false),
            "!" => !x.is_empty(),
            _ => false,
        },
        [a0, x, a2, y] if *a0 == "!" && *a2 == "=" => x != y,
        _ => false,
    }
}

fn newer(m1: &std::fs::Metadata, m2: &std::fs::Metadata) -> bool {
    m1.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        > m2.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH)
}

// ---------------------------------------------------------------------------
// interactive: line editor + tab completion
// ---------------------------------------------------------------------------

pub const PROMPT: &str = "sh# ";

/// 回 (word 起點, 原 word)。word = 光標前、以空白或控制字元分隔的片段。
pub fn word_bounds(line: &[char], pos: usize) -> (usize, String) {
    let mut start = pos;
    while start > 0 && !matches!(line[start - 1], ' ' | '\t' | '\n' | ';' | '|' | '&' | '<' | '>') {
        start -= 1;
    }
    let word: String = line[start..pos].iter().collect();
    (start, word)
}

/// 補全候選：first=true → 命令名（內建 + PATH 目錄執行檔）；
/// 否則補檔案路徑。lister 回目錄 entry 名稱清單（不含附錄 /）。
pub fn completion(
    word: &str,
    first: bool,
    lister: &dyn Fn(&str) -> Vec<String>,
    path_dirs: &[String],
) -> Vec<String> {
    let mut cands: Vec<String> = Vec::new();
    if first {
        for b in builtin_names() {
            if b.starts_with(word) {
                cands.push(b.to_string());
            }
        }
        for d in path_dirs {
            for e in lister(d) {
                if e.starts_with(word) {
                    cands.push(e);
                }
            }
        }
        cands.sort();
        cands.dedup();
        return cands;
    }
    // 路徑補全：dir 內符合 prefix 的 entry；目錄附 '/'
    let (dir, prefix) = match word.rsplit_once('/') {
        Some((d, p)) => (format!("{d}/"), p.to_string()),
        None => (String::new(), word.to_string()),
    };
    let lookup_dir = if word.contains('/') {
        if dir == "/" {
            "/"
        } else {
            &dir[..dir.len() - 1]
        }
    } else {
        "."
    };
    for e in lister(lookup_dir) {
        // lister 約定：目錄 entry 以 '/' 結尾（fs_lister 實作）
        if e.starts_with(&prefix) {
            cands.push(format!("{dir}{e}"));
        }
    }
    cands.sort();
    cands
}

pub fn fs_lister(dir: &str) -> Vec<String> {
    let mut v = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let isdir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            v.push(if isdir { format!("{name}/") } else { name });
        }
    }
    v.sort();
    v
}

pub fn path_dirs() -> Vec<String> {
    std::env::var("PATH")
        .unwrap_or_else(|_| "/bin:/usr/bin".to_string())
        .split(':')
        .map(str::to_string)
        .collect()
}

fn common_prefix(v: &[String]) -> String {
    if v.is_empty() {
        return String::new();
    }    let mut prefix: Vec<char> = v[0].chars().collect();
    for s in &v[1..] {
        let sc: Vec<char> = s.chars().collect();
        let mut n = 0;
        while n < prefix.len() && n < sc.len() && prefix[n] == sc[n] {
            n += 1;
        }
        prefix.truncate(n);
    }
    prefix.into_iter().collect()
}

/// 測试用可见性包装。
#[doc(hidden)]
pub fn common_prefix_for_test(v: &[String]) -> String {
    common_prefix(v)
}

impl<'a> Interp<'a> {
    fn interactive(&mut self) -> i32 {
        let _ = writeln!(
            self.err,
            "coreutils sh — minimal shell (Tab=completion, ↑/↓=history, Ctrl-D=exit)"
        );
        let mut history: Vec<String> = Vec::new();
        loop {
            let _ = write!(self.out, "{PROMPT}");
            let _ = self.out.flush();
            match self.read_line(&mut history) {
                None => {
                    let _ = writeln!(self.out);
                    break;
                }
                Some(line) => {
                    if line.trim().is_empty() {
                        continue;
                    }
                    history.push(line.clone());
                    if history.len() > 256 {
                        history.remove(0);
                    }
                    let code = self.run_text(&line);
                    self.last = code;
                }
            }
        }
        0
    }

    fn read_byte(&mut self) -> Option<u8> {
        let mut b = [0u8; 1];
        match self.inp.read(&mut b) {
            Ok(1) => Some(b[0]),
            _ => None,
        }
    }

    /// raw-mode 行編輯：可見字元插入、Backspace 刪除、↑↓ 歷史、←→ 移光標、
    /// Tab 補全、Ctrl-C 棄行、Ctrl-D EOF。回 None = EOF。
    fn read_line(&mut self, history: &mut Vec<String>) -> Option<String> {
        let saved = sys::raw_mode(sys::STDIN_FD);
        let result = self.read_line_inner(history);
        if let Some(s) = saved {
            sys::restore_mode(sys::STDIN_FD, &s);
        }
        result
    }

    fn read_line_inner(&mut self, history: &mut Vec<String>) -> Option<String> {
        let mut line: Vec<char> = Vec::new();
        let mut cur = 0usize;
        let mut hist_i = history.len();
        let mut stash: Option<String> = None;
        loop {
            let b = self.read_byte()?;
            match b {
                13 | 10 => {
                    self.echo("\n");
                    return Some(line.into_iter().collect());
                }
                3 => {
                    self.echo("^C\n");
                    return Some(String::new());
                }
                4 => {
                    if line.is_empty() {
                        self.echo("\n");
                        return None;
                    }
                }
                127 | 8 => {
                    if cur > 0 {
                        line.remove(cur - 1);
                        cur -= 1;
                        self.redraw(&line, cur);
                    }
                }
                9 => {
                    let (start, word) = word_bounds(&line, cur);
                    let first = line[..start].iter().all(|c| matches!(c, ' ' | '\t'));
                    let cands = completion(&word, first, &fs_lister, &path_dirs());
                    if cands.is_empty() {
                        self.echo("\u{7}"); // 無候選：bell
                        continue;
                    }
                    if cands.len() == 1 {
                        let repl = &cands[0];
                        line.splice(cur..cur, repl[word.len()..].chars());
                        cur += repl.len() - word.len();
                        self.redraw(&line, cur);
                    } else {
                        let cp = common_prefix(&cands);
                        if cp.len() > word.len() {
                            line.splice(cur..cur, cp[word.len()..].chars());
                            cur += cp.len() - word.len();
                            self.redraw(&line, cur);
                        } else {
                            self.echo("\n");
                            for c in cands.chunks(4) {
                                self.echo(&format!("  {}\n", c.join("  ")));
                            }
                            self.redraw(&line, cur);
                        }
                    }
                }
                27 => {
                    // escape sequence：只認 [A/B/C/D
                    let a = self.read_byte()?;
                    let c = self.read_byte()?;
                    if a == b'[' {
                        match c {
                            b'A' => {
                                if hist_i > 0 {
                                    if stash.is_none() {
                                        stash = Some(line.iter().collect());
                                    }
                                    hist_i -= 1;
                                    line = history[hist_i].chars().collect();
                                    cur = line.len();
                                    self.redraw(&line, cur);
                                }
                            }
                            b'B' => {
                                if hist_i < history.len() {
                                    hist_i += 1;
                                    line = match &stash {
                                        Some(s) if hist_i == history.len() => s.chars().collect(),
                                        _ if hist_i < history.len() => {
                                            history[hist_i].chars().collect()
                                        }
                                        _ => Vec::new(),
                                    };
                                    cur = line.len();
                                    self.redraw(&line, cur);
                                }
                            }
                            b'C' => cur = (cur + 1).min(line.len()),
                            b'D' => cur = cur.saturating_sub(1),
                            _ => {}
                        }
                        if matches!(c, b'C' | b'D') {
                            self.redraw(&line, cur);
                        }
                    }
                }
                b if b >= 32 && b < 127 => {
                    line.insert(cur, b as char);
                    cur += 1;
                    self.redraw(&line, cur);
                }
                _ => {}
            }
        }
    }

    fn echo(&mut self, s: &str) {
        let _ = write!(self.out, "{s}");
        let _ = self.out.flush();
    }

    /// 重畫整行：CR + prompt + 內容 + 清除到行尾，再把光標移回 cur。
    fn redraw(&mut self, line: &[char], cur: usize) {
        let content: String = line.iter().collect();
        let back = line.len() - cur;
        let _ = write!(self.out, "\r\u{1b}[K{PROMPT}{content}");
        if back > 0 {
            let _ = write!(self.out, "\u{1b}[{back}D");
        }
        let _ = self.out.flush();
    }
}
