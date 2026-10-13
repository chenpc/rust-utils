use coreutils::testutil as t;

fn count(args: &[&str]) -> usize {
    let r = t::run("nproc", args, "");
    assert_eq!(r.code, 0, "args {:?}", args);
    r.out().trim_end().parse::<usize>().unwrap()
}

fn with_env<T>(key: &str, value: &str, f: T)
where
    T: FnOnce(),
{
    let prev = std::env::var(key).ok();
    std::env::set_var(key, value);
    f();
    match prev {
        Some(v) => std::env::set_var(key, v),
        None => std::env::remove_var(key),
    }
}

#[test]
fn baseline_is_affinity_count() {
    let n = count(&[]);
    assert!(n >= 1);
    assert_eq!(count(&["--all"]), count(&[]).max(1));
    assert_eq!(count(&["--ignore", "0"]), n);
    assert_eq!(count(&["--ignore", "99"]), 1);
    assert!(count(&["--ignore", "1"]) >= 1);
    assert_eq!(count(&["--ignore=1"]), count(&[]).saturating_sub(1).max(1));
}

#[test]
fn omp_num_threads_replaces_the_count() {
    with_env("OMP_NUM_THREADS", "3", || {
        assert_eq!(count(&[]), 3);
        assert_eq!(count(&["--ignore", "1"]), 2);
        assert_eq!(count(&["--ignore", "9"]), 1);
    });
    with_env("OMP_NUM_THREADS", "99", || {
        assert_eq!(count(&[]), 99);
        assert_eq!(count(&["--ignore", "3"]), 96);
    });
    with_env("OMP_NUM_THREADS", "0", || {
        assert_eq!(count(&[]), count_with_env_unset());
    });
    with_env("OMP_NUM_THREADS", "abc", || {
        assert_eq!(count(&[]), count_with_env_unset());
    });
}

fn count_with_env_unset() -> usize {
    let saved = std::env::var("OMP_NUM_THREADS").ok();
    std::env::remove_var("OMP_NUM_THREADS");
    let n = count(&[]);
    if let Some(v) = saved {
        std::env::set_var("OMP_NUM_THREADS", v);
    }
    n
}

#[test]
fn omp_thread_limit_caps_after_ignore() {
    with_env("OMP_THREAD_LIMIT", "2", || {
        assert!(count(&[]) <= 2);
        assert_eq!(count(&["--ignore", "99"]), 1);
    });
    with_env("OMP_NUM_THREADS", "8", || {
        with_env("OMP_THREAD_LIMIT", "6", || {
            assert_eq!(count(&["--ignore", "3"]), 5);
        });
        with_env("OMP_THREAD_LIMIT", "3", || {
            assert_eq!(count(&[]), 3);
        });
    });
    with_env("OMP_THREAD_LIMIT", "0", || {
        assert_eq!(count(&[]), baseline_unset());
    });
}

fn baseline_unset() -> usize {
    let saved = std::env::var("OMP_THREAD_LIMIT").ok();
    std::env::remove_var("OMP_THREAD_LIMIT");
    let n = count(&[]);
    if let Some(v) = saved {
        std::env::set_var("OMP_THREAD_LIMIT", v);
    }
    n
}

#[test]
fn clap_style_errors() {
    let r = t::run("nproc", &["--ignore", "abc"], "");
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err(),
        "error: invalid value 'abc' for '--ignore <N>': invalid digit found in string\n\nFor more information, try '--help'.\n"
    );
    let r = t::run("nproc", &["--ignore"], "");
    assert_eq!(
        r.err(),
        "error: a value is required for '--ignore <N>' but none was supplied\n\nFor more information, try '--help'.\n"
    );
    let r = t::run("nproc", &["--ignore", "2", "--ignore", "3"], "");
    assert_eq!(
        r.err().lines().next().unwrap(),
        "error: the argument '--ignore <N>' cannot be used multiple times"
    );
    let r = t::run("nproc", &["--all", "--all"], "");
    assert_eq!(
        r.err().lines().next().unwrap(),
        "error: the argument '--all' cannot be used multiple times"
    );
    let r = t::run("nproc", &["5"], "");
    assert_eq!(r.err().lines().next().unwrap(), "error: unexpected argument '5' found");
    let r = t::run("nproc", &["-x"], "");
    assert_eq!(r.err().lines().next().unwrap(), "error: unexpected argument '-x' found");
}

#[test]
fn help_and_version() {
    let h = t::run("nproc", &["--help"], "").out();
    assert!(h.starts_with("Print the number of cores available to the current process.\n"));
    assert!(h.contains("      --ignore <N>       ignore up to N cores [default: 0]\n"));
    assert!(h.ends_with("they will determine the minimum and maximum returned value respectively.\n"));
    assert_eq!(t::run("nproc", &["--version"], "").out(), "nproc (uutils coreutils) 0.10.0\n");
}

#[test]
fn fuzz_invariants() {
    // 不變量：nproc 基礎值 N；--ignore k（合法）→ max(0, N-k)；非法 ignore → rc 2；
    // 任意組合的成功輸出恆為單一數字 token。
    let n: i64 = t::run("nproc", &[], "").out().trim().parse().expect("nproc base");
    assert!(n >= 1);
    let mut st: u64 = 0x4C21_9AC0;
    for _ in 0..50 {
        st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let k = ((st >> 30) % 10) as i64;
        let mode = (st >> 50) % 3;
        match mode {
            0 => {
                let r = t::run("nproc", &["--ignore", &k.to_string()], "");
                assert_eq!(r.code, 0);
                let v: i64 = r.out().trim().parse().expect("nproc ignore");
                assert_eq!(v, (n - k).max(1), "ignore {k}"); // 我方 E3 決定：下限 1
            }
            1 => {
                let all = t::run("nproc", &["--all"], "");
                assert_eq!(all.code, 0);
                assert_eq!(all.out().trim().parse::<i64>().unwrap(), n);
            }
            _ => {
                let bad = ["abc", "", "-1x", "1.5"][((st >> 20) % 4) as usize];
                let r = t::run("nproc", &["--ignore", bad], "");
                assert!(r.code == 2 || r.code == 1, "bad {bad:?} -> {} err={}", r.code, r.err());
                assert_eq!(r.out(), "");
            }
        }
    }
}
