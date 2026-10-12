use coreutils::sys;
use coreutils::testutil as t;

fn want(flags: &[char]) -> String {
    let u = sys::utsname().expect("uname(2)");
    let mut out: Vec<String> = Vec::new();
    for c in ['s', 'n', 'r', 'v', 'm', 'o'] {
        if flags.contains(&c) {
            let value = match c {
                's' => sys::field_str(&u.sysname),
                'n' => sys::field_str(&u.nodename),
                'r' => sys::field_str(&u.release),
                'v' => sys::field_str(&u.version),
                'm' => sys::field_str(&u.machine),
                _ => "GNU/Linux".to_string(),
            };
            out.push(value);
        }
    }
    out.join(" ")
}

fn out(args: &[&str]) -> String {
    let r = t::run("uname", args, "");
    assert_eq!(r.code, 0, "args {:?}", args);
    r.out().trim_end().to_string()
}

#[test]
fn default_is_kernel_name() {
    assert_eq!(out(&[]), want(&['s']));
    assert_eq!(out(&["-s"]), want(&['s']));
    assert_eq!(out(&["--kernel-name"]), want(&['s']));
}

#[test]
fn individual_fields() {
    assert_eq!(out(&["-n"]), want(&['n']));
    assert_eq!(out(&["-m"]), want(&['m']));
    assert_eq!(out(&["-r"]), want(&['r']));
    assert_eq!(out(&["-v"]), want(&['v']));
    assert_eq!(out(&["-o"]), "GNU/Linux");
    assert_eq!(sys::field_str(&sys::utsname().unwrap().machine), "x86_64");
}

#[test]
fn combined_output_follows_canonical_order() {
    assert_eq!(out(&["-s", "-m"]), want(&['s', 'm']));
    assert_eq!(out(&["-m", "-s"]), want(&['s', 'm']));
    assert_eq!(out(&["-om"]), want(&['m', 'o']));
    assert_eq!(out(&["-o", "-s", "-n"]), want(&['s', 'n', 'o']));
    assert_eq!(out(&["-sn"]), want(&['s', 'n']));
}

#[test]
fn all_expands_to_six_fields_in_order() {
    let full = want(&['s', 'n', 'r', 'v', 'm', 'o']);
    assert_eq!(out(&["-a"]), full);
    assert_eq!(out(&["--all"]), full);
    assert_eq!(out(&["-ma"]), full);
    assert_eq!(out(&["-a", "-s"]), full);
    assert_eq!(out(&["-s", "-a"]), full);
}

#[test]
fn duplicates_are_rejected() {
    let r = t::run("uname", &["--all", "-a"], "");
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err(),
        "error: the argument '--all' cannot be used multiple times\n\nFor more information, try '--help'.\n"
    );
    for (args, long) in
        [("-s", "--kernel-name"), ("-m", "--machine"), ("-n", "--nodename"), ("-o", "--operating-system")]
            .iter()
    {
        let r = t::run("uname", &[*args, *args], "");
        assert_eq!(r.code, 1, "args {:?}", args);
        assert_eq!(
            r.err().lines().next().unwrap(),
            format!("error: the argument '{}' cannot be used multiple times", long)
        );
    }
    let r = t::run("uname", &["-s", "--kernel-name"], "");
    assert_eq!(
        r.err().lines().next().unwrap(),
        "error: the argument '--kernel-name' cannot be used multiple times"
    );
}

#[test]
fn unknown_argument_is_a_clap_error_without_tip() {
    let r = t::run("uname", &["-x"], "");
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err(),
        "error: unexpected argument '-x' found\n\nUsage: uname [OPTION]...\n\nFor more information, try '--help'.\n"
    );
    let r = t::run("uname", &["foo"], "");
    assert_eq!(
        r.err(),
        "error: unexpected argument 'foo' found\n\nUsage: uname [OPTION]...\n\nFor more information, try '--help'.\n"
    );
}

#[test]
fn help_and_version() {
    let h = t::run("uname", &["--help"], "").out();
    assert!(h.starts_with("Print certain system information.\nWith no OPTION, same as -s.\n\nUsage: uname [OPTION]...\n"));
    assert!(h.contains("  -a, --all               Behave as though all of the options -mnrsvo were specified.\n"));
    assert!(h.contains("  -o, --operating-system  print the operating system name.\n"));
    assert!(h.contains("  -V, --version           Print version\n"));
    assert_eq!(t::run("uname", &["--version"], "").out(), "uname (uutils coreutils) 0.10.0\n");
    assert_eq!(t::run("uname", &["-V"], "").out(), "uname (uutils coreutils) 0.10.0\n");
    assert_eq!(t::run("uname", &["-h"], "").out(), h);
}

#[test]
fn arguments_after_double_dash_are_unexpected() {
    let r = t::run("uname", &["--", "-s"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("unexpected argument '-s' found"));
    let r = t::run("uname", &["--"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out().trim_end(), want(&['s']));
}

#[test]
fn fuzz_field_combinations() {
    let mut rng = t::Rng::new(20261201);
    let flags = ['s', 'n', 'r', 'v', 'm', 'o'];
    let names = ["-s", "-n", "-r", "-v", "-m", "-o"];
    for _ in 0..12 {
        let mut argv: Vec<&str> = Vec::new();
        let mut chars: Vec<char> = Vec::new();
        for (i, c) in flags.iter().enumerate() {
            if rng.below(2) == 0 {
                argv.push(names[i]);
                chars.push(*c);
            }
        }
        if chars.is_empty() {
            continue;
        }
        let mut canon: Vec<char> = ['s', 'n', 'r', 'v', 'm', 'o']
            .iter()
            .copied()
            .filter(|c| chars.contains(c))
            .collect();
        canon.sort();
        assert_eq!(out(&argv), want(&canon), "argv {:?}", argv);
    }
}
