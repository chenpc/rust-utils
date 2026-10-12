use crate::sys;
use crate::utmp;
use crate::{In, Out};

const WHO_HELP: &str = r#"Print information about the users currently on the machine, and their processes.

Usage: who [OPTION]... [ FILE | ARG1 ARG2 ]

Options:
  -a, --all         same as -b -d --login -p -r -t -T -u
  -b, --boot        time of last system boot
  -d, --dead        print dead processes
  -H, --heading     print line of column headings
  -l, --login       print system login processes
      --lookup      attempt to canonicalize hostnames via DNS
  -m                only hostname and user associated with stdin
  -p, --process     print active processes spawned by init
  -q, --count       all login names and number of users logged on
  -r, --runlevel    print current runlevel
  -s, --short       print only name, line, and time (default)
  -t, --time        print last system clock change
  -T, -W, --mesg    add user's message status as +, - or ?
  -u, --users       list users logged in
      --login       list users logged in
  -h, --help        Print help
  -V, --version     Print version
"#;

fn unexpected(arg: &str, stderr: Out) -> i32 {
    write!(
        stderr,
        "error: unexpected argument '{}' found\n\n  tip: to pass '{}' as a value, use '-- {}'\n\nUsage: who [OPTION]... [ FILE | ARG1 ARG2 ]\n\nFor more information, try '--help'.\n",
        arg, arg, arg
    )
    .ok();
    1
}

fn dup(name: &str, stderr: Out) -> i32 {
    write!(
        stderr,
        "error: the argument '--{}' cannot be used multiple times\n\nFor more information, try '--help'.\n",
        name
    )
    .ok();
    1
}

fn flag_seen(seen: &[&str], name: &str) -> bool {
    seen.iter().any(|s| *s == name)
}

struct Cols {
    name_w: usize,
    mesg: bool,
    idle: bool,
    pid: bool,
    comment_w: Option<usize>,
    exit: bool,
}

fn cols_for(o: &Opts) -> Cols {
    let extended = o.all || o.users || o.dead || o.login || o.runlevel || o.process || o.boot
        || o.clock;
    Cols {
        name_w: if o.all || o.mesg { 10 } else { 8 },
        mesg: o.all || o.mesg,
        idle: extended,
        pid: extended,
        comment_w: if o.all || o.dead { Some(8) } else { None },
        exit: o.all || o.dead,
    }
}

fn heading(o: &Opts) -> &'static str {
    // Column headings are fixed strings in the reference; these are its exact
    // renderings for each option group (measured, see docs/assumptions.md).
    if o.all {
        "NAME       LINE         TIME       IDLE          PID COMMENT  EXIT"
    } else if o.dead {
        "NAME     LINE         TIME       IDLE          PID COMMENT  EXIT"
    } else if o.users || o.login || o.runlevel {
        "NAME     LINE         TIME       IDLE          PID COMMENT"
    } else if o.process || o.boot || o.clock {
        "NAME     LINE         TIME              PID COMMENT"
    } else if o.mesg {
        "NAME       LINE         TIME       COMMENT"
    } else {
        "NAME     LINE         TIME       COMMENT"
    }
}

fn width_list(c: &Cols) -> Vec<usize> {
    let mut w = vec![c.name_w, 12, 10];
    if c.idle {
        w.push(13);
    }
    if c.pid {
        w.push(3);
    }
    w.push(c.comment_w.unwrap_or(0));
    if c.exit {
        w.push(0);
    }
    w
}

fn join_fields(fields: &[String], widths: &[usize]) -> String {
    let mut out = String::new();
    for (i, f) in fields.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(f);
        if i < widths.len() && widths[i] > f.chars().count() {
            for _ in 0..(widths[i] - f.chars().count()) {
                out.push(' ');
            }
        }
    }
    while out.ends_with(' ') {
        out.pop();
    }
    out
}

fn fmt_time(secs: i32) -> String {
    match sys::local_time(secs as i64) {
        Some((y, mo, d, h, mi)) => format!("{:04}-{:02}-{:02} {:02}:{:02}", y, mo, d, h, mi),
        None => String::new(),
    }
}

fn idle_text(secs: i32, now: i64) -> String {
    let delta = now - secs as i64;
    if delta < 0 {
        return "00:00".to_string();
    }
    if delta > 86400 {
        "old".to_string()
    } else {
        format!("{:02}:{:02}", delta / 3600, (delta % 3600) / 60)
    }
}


struct Opts {
    all: bool,
    boot: bool,
    dead: bool,
    heading: bool,
    login: bool,
    #[allow(dead_code)]
    lookup: bool,
    only_stdin: bool,
    process: bool,
    count: bool,
    runlevel: bool,
    clock: bool,
    users: bool,
    mesg: bool,
    file: String,
}

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut o = Opts {
        all: false,
        boot: false,
        dead: false,
        heading: false,
        login: false,
        lookup: false,
        only_stdin: false,
        process: false,
        count: false,
        runlevel: false,
        clock: false,
        users: false,
        mesg: false,
        file: utmp::DEFAULT_PATH.to_string(),
    };
    let mut seen: Vec<String> = Vec::new();
    let mut only_positional = false;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        i += 1;
        if only_positional {
            if o.file == utmp::DEFAULT_PATH {
                o.file = a.clone();
                continue;
            }
            return unexpected(a, stderr);
        }
        if a == "--" {
            only_positional = true;
            continue;
        }
        if a == "--help" || a == "-h" || a == "-hV" {
            write!(stdout, "{}", WHO_HELP).ok();
            return 0;
        }
        if a == "--version" || a == "-V" || a == "-Vh" {
            writeln!(stdout, "who (uutils coreutils) 0.10.0").ok();
            return 0;
        }
        if a.starts_with("--") {
            let name = a[2..].split('=').next().unwrap_or("").to_string();
            let short: Option<(&str, char)> = match name.as_str() {
                "all" => Some(("all", 'a')),
                "boot" => Some(("boot", 'b')),
                "dead" => Some(("dead", 'd')),
                "heading" => Some(("heading", 'H')),
                "login" => Some(("login", 'l')),
                "lookup" => Some(("lookup", 'k')),
                "process" => Some(("process", 'p')),
                "count" => Some(("count", 'q')),
                "runlevel" => Some(("runlevel", 'r')),
                "short" => Some(("short", 's')),
                "time" => Some(("time", 't')),
                "mesg" => Some(("mesg", 'T')),
                "users" => Some(("users", 'u')),
                "help" => {
                    write!(stdout, "{}", WHO_HELP).ok();
                    return 0;
                }
                "version" => {
                    writeln!(stdout, "who (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                _ => return unexpected(a, stderr),
            };
            let (long, c) = short.unwrap();
            if flag_seen(&seen.iter().map(|s| s.as_str()).collect::<Vec<&str>>(), long) {
                return dup(long, stderr);
            }
            seen.push(long.to_string());
            apply(&mut o, c);
            continue;
        }
        if a.starts_with('-') && a.len() > 1 {
                    for k in 1..a.len() {
                let c = a.as_bytes()[k] as char;
                let long = match c {
                    'a' => "all",
                    'b' => "boot",
                    'd' => "dead",
                    'H' => "heading",
                    'l' => "login",
                    'm' => "lookup",
                    'p' => "process",
                    'q' => "count",
                    'r' => "runlevel",
                    's' => "short",
                    't' => "time",
                    'T' | 'W' => "mesg",
                    'u' => "users",
                    'h' => {
                        write!(stdout, "{}", WHO_HELP).ok();
                        return 0;
                    }
                    'V' => {
                        writeln!(stdout, "who (uutils coreutils) 0.10.0").ok();
                        return 0;
                    }
                    _ => return unexpected(&format!("-{}", c), stderr),
                };
                if flag_seen(&seen.iter().map(|s| s.as_str()).collect::<Vec<&str>>(), long) {
                    return dup(long, stderr);
                }
                seen.push(long.to_string());
                apply(&mut o, c);
            }
            continue;
        }
        if o.file == utmp::DEFAULT_PATH {
            o.file = a.clone();
            continue;
        }
        return unexpected(a, stderr);
    }

    let mut records: Vec<&utmp::Record> = Vec::new();
    let all_records = utmp::read_records(&o.file);
    let mut selected: Vec<i16> = Vec::new();
    if o.all {
        selected.extend([
            utmp::RUN_LVL,
            utmp::BOOT_TIME,
            utmp::NEW_TIME,
            utmp::OLD_TIME,
            utmp::INIT_PROCESS,
            utmp::LOGIN_PROCESS,
            utmp::USER_PROCESS,
            utmp::DEAD_PROCESS,
        ]);
    } else {
        let any_special = o.boot || o.dead || o.login || o.process || o.runlevel || o.clock;
        if !any_special || o.users {
            selected.push(utmp::USER_PROCESS);
        }
        if o.boot {
            selected.push(utmp::BOOT_TIME);
        }
        if o.dead {
            selected.push(utmp::DEAD_PROCESS);
        }
        if o.login {
            selected.push(utmp::LOGIN_PROCESS);
        }
        if o.process {
            selected.push(utmp::INIT_PROCESS);
        }
        if o.runlevel {
            selected.push(utmp::RUN_LVL);
        }
        if o.clock {
            selected.extend([utmp::NEW_TIME, utmp::OLD_TIME]);
        }
    }
    for r in &all_records {
        if selected.contains(&r.utype) {
            records.push(r);
        }
    }
    if o.only_stdin {
        let tty = sys::tty_name();
        records.retain(|r| match &tty {
            Some(t) => r.line == *t,
            None => false,
        });
    }

    if o.count {
        let names: Vec<String> = records
            .iter()
            .filter(|r| r.utype == utmp::USER_PROCESS)
            .map(|r| r.user.clone())
            .collect();
        if names.is_empty() {
            writeln!(stdout).ok();
        }
        for chunk in names.chunks(14) {
            writeln!(stdout, "{}", chunk.join(" ")).ok();
        }
        writeln!(stdout, "# users={}", names.len()).ok();
        return 0;
    }

    let cols = cols_for(&o);
    let widths = width_list(&cols);
    if o.heading {
        writeln!(stdout, "{}", heading(&o)).ok();
    }
    let now = sys::now_epoch();
    for r in records {
        let mut fields: Vec<String> = Vec::new();
        let is_process = !matches!(
            r.utype,
            utmp::BOOT_TIME | utmp::RUN_LVL | utmp::NEW_TIME | utmp::OLD_TIME
        );
        let mut name = if is_process { r.user.clone() } else { String::new() };
        if cols.mesg && is_process {
            while name.chars().count() < cols.name_w - 1 {
                name.push(' ');
            }
            name.push(mesg_char(&r.line));
        }
        fields.push(name);
        fields.push(match r.utype {
            utmp::BOOT_TIME => "system boot".to_string(),
            utmp::RUN_LVL => format!("run-level {}", r.id),
            utmp::NEW_TIME | utmp::OLD_TIME => "clock change".to_string(),
            _ => r.line.clone(),
        });
        fields.push(fmt_time(r.tv_sec));
        if cols.idle {
            fields.push(match r.utype {
                utmp::DEAD_PROCESS => "gone".to_string(),
                utmp::USER_PROCESS if o.users || o.all => idle_text(r.tv_sec, now),
                _ => String::new(),
            });
        }
        if cols.pid {
            fields.push(match r.utype {
                utmp::BOOT_TIME | utmp::RUN_LVL | utmp::NEW_TIME | utmp::OLD_TIME => String::new(),
                _ => r.pid.to_string(),
            });
        }
        fields.push(r.host.clone());
        if cols.exit {
            fields.push(match r.utype {
                utmp::DEAD_PROCESS => format!("term={} exit={}", r.term_status, r.exit_status),
                _ => String::new(),
            });
        }
        writeln!(stdout, "{}", join_fields(&fields, &widths)).ok();
    }
    0
}

fn apply(o: &mut Opts, c: char) {
    match c {
        'a' => o.all = true,
        'b' => o.boot = true,
        'd' => o.dead = true,
        'H' => o.heading = true,
        'l' => o.login = true,
        'k' => o.lookup = true,
        'm' => o.only_stdin = true,
        'p' => o.process = true,
        'q' => o.count = true,
        'r' => o.runlevel = true,
        's' => {}
        't' => o.clock = true,
        'T' | 'W' => o.mesg = true,
        'u' => o.users = true,
        _ => {}
    }
}

/// GNU reports the user's message status as +, - or ?: unknown when the
/// terminal device cannot be inspected.
fn mesg_char(line: &str) -> char {
    if line.is_empty() {
        return '?';
    }
    let path = format!("/dev/{}", line);
    match std::fs::metadata(&path) {
        Ok(md) => {
            use std::os::unix::fs::PermissionsExt;
            if md.permissions().mode() & 0o222 != 0 {
                '+'
            } else {
                '-'
            }
        }
        Err(_) => '?',
    }
}
