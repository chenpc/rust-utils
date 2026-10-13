// seq applet: print numbers from FIRST to LAST in steps of INCREMENT.
//
// Option parsing, error wording and number formatting are aligned with the
// reference binary (uutils coreutils 0.10.0) by black-box probing; the
// observations live in docs/probes/seq.md. Calibrated points:
//   * options are honoured only before the first value; afterwards every token
//     (even `-w`, `--help`, `--version`) is a value, and a 4th value errors.
//   * decimal places come from FIRST and INCREMENT only, never from LAST.
//   * the final value is decided by exact decimal arithmetic, not by f64.

use crate::pathname::usage_hint;

const SEQ_USAGE: &str = concat!(
    "Usage: seq [OPTION]... LAST\n",
    "       seq [OPTION]... FIRST LAST\n",
    "       seq [OPTION]... FIRST INCREMENT LAST\n"
);

const HELP_TEXT: &str = concat!(
    "Display numbers from FIRST to LAST, in steps of INCREMENT.\n",
    "\n",
    "Usage: seq [OPTION]... LAST\n",
    "       seq [OPTION]... FIRST LAST\n",
    "       seq [OPTION]... FIRST INCREMENT LAST\n",
    "\n",
    "Arguments:\n",
    "  [numbers]...  \n",
    "\n",
    "Options:\n",
    "  -s, --separator <separator>    Separator character (defaults to \\n)\n",
    "  -t, --terminator <terminator>  Terminator character (defaults to \\n)\n",
    "  -w, --equal-width              Equalize widths of all numbers by padding with zeros\n",
    "  -f, --format <format>          use printf style floating-point FORMAT\n",
    "  -h, --help                     Print help\n",
    "  -V, --version                  Print version\n"
);

pub fn run(
    args: &[String],
    _stdin: crate::In,
    stdout: crate::Out,
    stderr: crate::Out,
) -> i32 {
    match run_inner(args, stdout, stderr) {
        Ok(code) => code,
        Err(e) => {
            let _ = stdout.flush();
            let _ = writeln!(stderr, "seq: {}", e);
            let _ = writeln!(stderr, "{}", usage_hint("seq"));
            1
        }
    }
}

/// Decimal literal held exactly: value = digits * 10^(point - digits.len()).
#[derive(Clone)]
struct Dec {
    text: String,
    neg: bool,
    digits: Vec<u8>,
    point: i64,
    value: f64,
}

impl Dec {
    /// Decimal places needed to write this literal (negative => integer zeros).
    fn frac(&self) -> i64 {
        self.digits.len() as i64 - self.point
    }

    fn decimals(&self) -> i64 {
        self.frac().max(0)
    }

    /// value * 10^scale as an integer, when it fits.
    fn scaled(&self, scale: i64) -> Option<i128> {
        let shift = scale - self.frac();
        if shift < 0 || shift > 38 {
            return None;
        }
        let mut v: i128 = 0;
        for &c in &self.digits {
            v = v.checked_mul(10)?.checked_add((c - b'0') as i128)?;
        }
        for _ in 0..shift {
            v = v.checked_mul(10)?;
        }
        Some(if self.neg { -v } else { v })
    }
}

fn dec_parse(text: &str) -> Option<Dec> {
    if text.is_empty() {
        return None;
    }
    let lower = text.to_ascii_lowercase();
    let special = match lower.as_str() {
        "nan" => Some(f64::NAN),
        "inf" | "infinity" => Some(f64::INFINITY),
        "-inf" | "-infinity" => Some(f64::NEG_INFINITY),
        "+inf" | "+infinity" => Some(f64::INFINITY),
        _ => None,
    };
    if let Some(v) = special {
        return Some(Dec {
            text: text.to_string(),
            neg: lower.starts_with('-'),
            digits: Vec::new(),
            point: 0,
            value: v,
        });
    }
    let (neg, rest) = match text.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    if rest.is_empty() {
        return None;
    }
    if rest.starts_with("0x") || rest.starts_with("0X") {
        return hex_parse(rest, neg, text);
    }
    if rest.starts_with("0x") || rest.starts_with("0X") {
        return hex_parse(rest, neg, text);
    }
    let (mant, exp_part) = match rest.find(['e', 'E']) {
        Some(i) => (&rest[..i], &rest[i + 1..]),
        None => (rest, ""),
    };
    let exp10: i64 = if exp_part.is_empty() {
        0
    } else {
        let (esign, edigits) = match exp_part.strip_prefix('-') {
            Some(r) => (-1i64, r),
            None => (1i64, exp_part.strip_prefix('+').unwrap_or(exp_part)),
        };
        if edigits.is_empty() || !edigits.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        match edigits.parse::<i64>() {
            Ok(v) => esign * v,
            Err(_) => return None,
        }
    };
    let mut int_part = String::new();
    let mut frac_part = String::new();
    let mut after_point = false;
    let mut seen_point = false;
    for c in mant.chars() {
        if c == '.' {
            if seen_point {
                return None;
            }
            seen_point = true;
            after_point = true;
            continue;
        }
        if !c.is_ascii_digit() {
            return None;
        }
        if after_point {
            frac_part.push(c);
        } else {
            int_part.push(c);
        }
    }
    if int_part.is_empty() && frac_part.is_empty() {
        return None;
    }
    let digits_raw = format!("{}{}", int_part, frac_part);
    let trimmed = digits_raw.trim_start_matches('0');
    let lead_zeros = digits_raw.len() - trimmed.len();
    let point = int_part.len() as i64 + exp10 - lead_zeros as i64;
    Some(Dec {
        text: text.to_string(),
        neg,
        digits: trimmed.as_bytes().to_vec(),
        point,
        value: text.parse::<f64>().ok()?,
    })
}

#[derive(PartialEq)]
enum Action {
    Help,
    Version,
}

/// Multiply a decimal digit vector (most significant digit first) by a single
/// decimal digit, in place.
fn mul_digit(digits: &mut Vec<u8>, f: u8) {
    let mut carry = 0u8;
    for d in digits.iter_mut().rev() {
        let v = *d as u32 * f as u32 + carry as u32;
        *d = (v % 10) as u8;
        carry = (v / 10) as u8;
    }
    while carry > 0 {
        digits.insert(0, (carry % 10) as u8);
        carry /= 10;
    }
}

/// Hexadecimal floating point literals (`0x10`, `0x1.8p2`) are accepted by both
/// references because operands go through strtod. The value is kept exact:
/// N * 2^e is rewritten as a decimal digit string with `point` digits before
/// the decimal point.
fn hex_parse(body: &str, neg: bool, text: &str) -> Option<Dec> {
    let body = &body[2..];
    let (mant, exp_part) = match body.find(['p', 'P']) {
        Some(i) => (&body[..i], &body[i + 1..]),
        None => (body, ""),
    };
    let exp2: i64 = if exp_part.is_empty() {
        0
    } else {
        let (esign, edigits) = match exp_part.strip_prefix('-') {
            Some(r) => (-1i64, r),
            None => (1i64, exp_part.strip_prefix('+').unwrap_or(exp_part)),
        };
        if edigits.is_empty() || !edigits.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        match edigits.parse::<i64>() {
            Ok(v) => esign * v,
            Err(_) => return None,
        }
    };
    let mut int_hex = String::new();
    let mut frac_hex = String::new();
    let mut after_point = false;
    let mut seen_point = false;
    for c in mant.chars() {
        if c == '.' {
            if seen_point {
                return None;
            }
            seen_point = true;
            after_point = true;
            continue;
        }
        if !c.is_ascii_hexdigit() {
            return None;
        }
        if after_point {
            frac_hex.push(c);
        } else {
            int_hex.push(c);
        }
    }
    if int_hex.is_empty() && frac_hex.is_empty() {
        return None;
    }
    if int_hex.len() + frac_hex.len() > 32 {
        return None;
    }
    let mut n: u128 = 0;
    for c in int_hex.chars().chain(frac_hex.chars()) {
        n = n.checked_mul(16)?.checked_add(c.to_digit(16)? as u128)?;
    }
    // value = n * 2^exp2 / 16^(frac hex digits)
    let mut digits: Vec<u8> = n.to_string().bytes().map(|c| c - b'0').collect();
    let shift = 4 * frac_hex.len() as i64 - exp2;
    if shift > 4096 || shift < -4096 {
        return None;
    }
    let point;
    if shift >= 0 {
        for _ in 0..shift {
            mul_digit(&mut digits, 5);
        }
        // n * 5^shift / 10^shift: integer part keeps the original width
        point = digits.len() as i64 - shift;
    } else {
        for _ in 0..(-shift) {
            mul_digit(&mut digits, 2);
        }
        point = digits.len() as i64;
    }
    // drop trailing zeros that sit after the decimal point (value unchanged)
    while digits.len() as i64 > point.max(1) && digits.last() == Some(&0) {
        digits.pop();
    }
    // f64 value: single rounding of the exact mantissa, then an exact power of
    // two (same rounding behaviour as C strtod on hex literals).
    let shift_exp = exp2 - 4 * frac_hex.len() as i64;
    let mut value = n as f64;
    let mut e = shift_exp;
    while e > 1000 {
        value *= 2f64.powi(1000);
        e -= 1000;
    }
    while e < -1000 {
        value *= 2f64.powi(-1000);
        e += 1000;
    }
    value *= 2f64.powi(e as i32);
    Some(Dec {
        text: text.to_string(),
        neg,
        // Dec stores ASCII digits (see dec_parse / Dec::scaled).
        digits: digits.iter().map(|d| b'0' + d).collect(),
        point,
        value: if neg { -value } else { value },
    })
}

/// Hex operands reach the reference through its float path, so they do not take
/// part in the `-w` zero padding width (`seq -w 0x10` prints unpadded 1..16).
fn is_hex_operand(text: &str) -> bool {
    let body = text.strip_prefix(['-', '+']).unwrap_or(text);
    body.starts_with("0x") || body.starts_with("0X")
}

fn field_of(d: &Dec, rendered: impl FnOnce() -> String) -> usize {
    if is_hex_operand(&d.text) {
        0
    } else {
        int_field(&rendered())
    }
}

/// Hex literal with a fractional part (0x1.8, 0x1p-4): the reference drops the
/// exact-decimal path for these and works in f64 instead.
fn is_hex_float(d: &Dec) -> bool {
    is_hex_operand(&d.text) && d.frac() > 0
}

struct Opts {
    sep: String,
    term: Option<String>,
    fmt: Option<String>,
    widths: bool,
    operands: Vec<String>,
    action: Option<Action>,
}

fn clap_error(msg: &str, usage: bool, stderr: crate::Out) {
    let _ = writeln!(stderr, "error: {}", msg);
    let _ = writeln!(stderr);
    if usage {
        let _ = write!(stderr, "{}", SEQ_USAGE);
        let _ = writeln!(stderr);
    }
    let _ = writeln!(stderr, "For more information, try '--help'.");
}

/// clap refuses to use a token as an option value when that token names a
/// known option (`-s --format 1 3` => "a value is required"); an unknown token
/// that merely looks like an option is taken verbatim (`-s -x` => separator `-x`).
fn known_option(token: &str) -> bool {
    if token == "-" || !token.starts_with('-') {
        return false;
    }
    if token.starts_with("--") {
        let name = token[2..].split('=').next().unwrap_or("");
        return match_long(name).is_some();
    }
    matches!(token[1..].chars().next(), Some('w' | 'h' | 'V' | 's' | 't' | 'f'))
}

fn value_required(long: &str, stderr: crate::Out) {
    clap_error(
        &format!(
            "a value is required for '--{} <{}>' but none was supplied",
            long, long
        ),
        false,
        stderr,
    );
}

fn duplicate(long: &str, metavar: Option<&str>, stderr: crate::Out) {
    let arg = match metavar {
        Some(m) => format!("--{} <{}>", long, m),
        None => format!("--{}", long),
    };
    clap_error(
        &format!("the argument '{}' cannot be used multiple times", arg),
        false,
        stderr,
    );
}

/// Long option names resolved by unique prefix, like clap does.
fn match_long(name: &str) -> Option<(&'static str, bool)> {
    const NAMES: [(&str, bool); 6] = [
        ("separator", true),
        ("terminator", true),
        ("equal-width", false),
        ("format", true),
        ("help", false),
        ("version", false),
    ];
    if name.is_empty() {
        return None;
    }
    let hits: Vec<(&'static str, bool)> = NAMES
        .iter()
        .filter(|(n, _)| n.starts_with(name))
        .map(|(n, v)| (*n, *v))
        .collect();
    if hits.len() == 1 {
        Some(hits[0])
    } else {
        None
    }
}

fn push_operand(opts: &mut Opts, token: &str, _stderr: crate::Out) -> Result<(), ()> {
    // clap keeps collecting trailing tokens; the surplus is reported once the
    // whole command line has been scanned (naming the last value).
    opts.operands.push(token.to_string());
    Ok(())
}

fn parse_args(args: &[String], stderr: crate::Out) -> Result<Opts, ()> {
    let mut opts = Opts {
        sep: "\n".to_string(),
        term: None,
        fmt: None,
        widths: false,
        operands: Vec::new(),
        action: None,
    };
    let mut seen_sep = false;
    let mut seen_term = false;
    let mut seen_fmt = false;
    let mut escape = false;
    let mut i = 0;
    while i < args.len() {
        let token = args[i].as_str();
        i += 1;
        if escape {
            push_operand(&mut opts, token, stderr)?;
            continue;
        }
        if !opts.operands.is_empty() {
            // clap keeps parsing options after positionals appear: a short option
            // with an inline value lands as two separate positionals, so
            // `seq 1 3 -s,` reports the attached value as the extra one.
            if token.len() > 2
                && !token.starts_with("--")
                && matches!(token.as_bytes()[1], b's' | b't' | b'f')
            {
                push_operand(&mut opts, &token[..2], stderr)?;
                push_operand(&mut opts, &token[2..], stderr)?;
            } else {
                push_operand(&mut opts, token, stderr)?;
            }
            continue;
        }
        if token == "--" {
            escape = true;
            continue;
        }
        if token == "-" || !token.starts_with('-') {
            push_operand(&mut opts, token, stderr)?;
            continue;
        }
        if token.starts_with("--") {
            let body = &token[2..];
            let (name, inline) = match body.find('=') {
                Some(p) => (&body[..p], Some(&body[p + 1..])),
                None => (body, None),
            };
            let (long, takes_value) = match match_long(name) {
                Some(v) => v,
                None => {
                    push_operand(&mut opts, token, stderr)?;
                    continue;
                }
            };
            let value = if takes_value {
                match inline {
                    Some(v) => Some(v.to_string()),
                    None => {
                        if i < args.len() && args[i] != "--" && !known_option(&args[i]) {
                            let v = args[i].clone();
                            i += 1;
                            Some(v)
                        } else {
                            None
                        }
                    }
                }
            } else {
                if let Some(v) = inline {
                    clap_error(
                        &format!("unexpected value '{}' for '--{}' found", v, long),
                        false,
                        stderr,
                    );
                    return Err(());
                }
                None
            };
            match long {
                "separator" => {
                    if seen_sep {
                        duplicate("separator", Some("separator"), stderr);
                        return Err(());
                    }
                    match value {
                        Some(v) => {
                            seen_sep = true;
                            opts.sep = v;
                        }
                        None => {
                            value_required("separator", stderr);
                            return Err(());
                        }
                    }
                }
                "terminator" => {
                    if seen_term {
                        duplicate("terminator", Some("terminator"), stderr);
                        return Err(());
                    }
                    match value {
                        Some(v) => {
                            seen_term = true;
                            opts.term = Some(v);
                        }
                        None => {
                            value_required("terminator", stderr);
                            return Err(());
                        }
                    }
                }
                "format" => {
                    if seen_fmt {
                        duplicate("format", Some("format"), stderr);
                        return Err(());
                    }
                    match value {
                        Some(v) => {
                            seen_fmt = true;
                            opts.fmt = Some(v);
                        }
                        None => {
                            value_required("format", stderr);
                            return Err(());
                        }
                    }
                }
                "equal-width" => {
                    if opts.widths {
                        duplicate("equal-width", None, stderr);
                        return Err(());
                    }
                    opts.widths = true;
                }
                "help" => {
                    opts.action = Some(Action::Help);
                    return Ok(opts);
                }
                "version" => {
                    opts.action = Some(Action::Version);
                    return Ok(opts);
                }
                _ => {}
            }
            continue;
        }
        let chars: Vec<char> = token.chars().skip(1).collect();
        let mut k = 0;
        let mut unknown = false;
        while k < chars.len() {
            match chars[k] {
                'w' => {
                    if opts.widths {
                        duplicate("equal-width", None, stderr);
                        return Err(());
                    }
                    opts.widths = true;
                }
                'h' => {
                    opts.action = Some(Action::Help);
                    return Ok(opts);
                }
                'V' => {
                    opts.action = Some(Action::Version);
                    return Ok(opts);
                }
                c @ ('s' | 't' | 'f') => {
                    let long = match c {
                        's' => "separator",
                        't' => "terminator",
                        _ => "format",
                    };
                    let seen = match c {
                        's' => &mut seen_sep,
                        't' => &mut seen_term,
                        _ => &mut seen_fmt,
                    };
                    if *seen {
                        duplicate(long, Some(long), stderr);
                        return Err(());
                    }
                    let rest: String = chars[k + 1..].iter().collect();
                    let value = if !rest.is_empty() {
                        Some(rest)
                    } else if i < args.len() && args[i] != "--" && !known_option(&args[i]) {
                        let v = args[i].clone();
                        i += 1;
                        Some(v)
                    } else {
                        None
                    };
                    match value {
                        Some(v) => {
                            *seen = true;
                            match c {
                                's' => opts.sep = v,
                                't' => opts.term = Some(v),
                                _ => opts.fmt = Some(v),
                            }
                        }
                        None => {
                            value_required(long, stderr);
                            return Err(());
                        }
                    }
                    break;
                }
                _ => {
                    unknown = true;
                    break;
                }
            }
            k += 1;
        }
        if unknown {
            push_operand(&mut opts, token, stderr)?;
        }
    }
    if opts.operands.len() > 3 {
        clap_error(
            &format!(
                "unexpected value '{}' for '[numbers]...' found; no more were expected",
                opts.operands[opts.operands.len() - 1]
            ),
            true,
            stderr,
        );
        return Err(());
    }
    Ok(opts)
}

fn unit() -> Dec {
    Dec {
        text: "1".to_string(),
        neg: false,
        digits: vec![b'1'],
        point: 1,
        value: 1.0,
    }
}

fn run_inner(args: &[String], stdout: crate::Out, stderr: crate::Out) -> Result<i32, String> {
    let opts = match parse_args(args, stderr) {
        Ok(o) => o,
        Err(()) => {
            let _ = stdout.flush();
            return Ok(1);
        }
    };
    match opts.action {
        Some(Action::Help) => {
            let _ = write!(stdout, "{}", HELP_TEXT);
            let _ = stdout.flush();
            return Ok(0);
        }
        Some(Action::Version) => {
            let _ = writeln!(stdout, "seq (uutils coreutils) 0.10.0");
            let _ = stdout.flush();
            return Ok(0);
        }
        None => {}
    }

    if opts.operands.is_empty() {
        return Err("missing operand".to_string());
    }
    if opts.widths && opts.fmt.is_some() {
        return Err(
            "format string may not be specified when printing equal width strings".to_string(),
        );
    }
    let mut lits: Vec<Dec> = Vec::new();
    for text in &opts.operands {
        match dec_parse(text) {
            Some(d) if !d.value.is_nan() => lits.push(d),
            Some(d) => {
                let _ = stdout.flush();
                let _ = writeln!(
                    stderr,
                    "seq: invalid 'not-a-number' argument: '{}'",
                    escape_for_display(&d.text)
                );
                let _ = writeln!(stderr, "{}", usage_hint("seq"));
                return Ok(1);
            }
            None => {
                return Err(format!(
                    "invalid floating point argument: '{}'",
                    escape_for_display(text)
                ));
            }
        }
    }
    let (first, increment, last) = match lits.len() {
        1 => (unit(), unit(), lits[0].clone()),
        2 => (lits[0].clone(), unit(), lits[1].clone()),
        _ => (lits[0].clone(), lits[1].clone(), lits[2].clone()),
    };
    if increment.value == 0.0 {
        return Err(format!(
            "invalid Zero increment value: '{}'",
            escape_for_display(&increment.text)
        ));
    }
    if let Some(spec) = opts.fmt.as_deref() {
        if let Err(msg) = validate_format(spec) {
            // Format diagnostics are reported without the clap "Try --help" hint.
            let _ = stdout.flush();
            let _ = writeln!(stderr, "seq: {}", msg);
            return Ok(1);
        }
    }
    let down = increment.neg;
    if first.value.is_finite() && last.value.is_finite() {
        let ord = cmp_decimal(&first, &last);
        if down {
            if ord < 0 {
                return Ok(0);
            }
        } else if ord > 0 {
            return Ok(0);
        }
    } else if down && first.value == f64::NEG_INFINITY && last.value == f64::NEG_INFINITY {
        return Ok(0);
    }

    // A hexadecimal operand with a fractional part reaches the reference through
    // its float path: counting stays exact, rendering uses C `%g` semantics.
    let hex_float = is_hex_float(&first) || is_hex_float(&increment) || is_hex_float(&last);
    let precision = first.decimals().max(increment.decimals());
    let scale = precision.max(last.decimals());
    let exact: Option<(i128, i128, i128, i64)> = if !hex_float
        && scale <= 18
        && first.value.is_finite()
        && increment.value.is_finite()
        && last.value.is_finite()
    {
        match (first.scaled(scale), increment.scaled(scale), last.scaled(scale)) {
            (Some(f), Some(i), Some(l)) => Some((f, i, l, scale)),
            _ => None,
        }
    } else {
        None
    };

    let term = opts.term.clone().unwrap_or_else(|| "\n".to_string());

    if let Some((f, i, l, sc)) = exact {
        let diff = l - f;
        let steps: i128 = if diff == 0 {
            0
        } else if (diff > 0 && i > 0) || (diff < 0 && i < 0) {
            diff / i
        } else {
            return Ok(0);
        };
        let drop = (sc - precision) as u32;
        let div = 10i128.pow(drop);
        let exact_at = |k: i128| -> Option<i128> { i.checked_mul(k).and_then(|d| f.checked_add(d)) };
        // A negative zero given as FIRST keeps its sign in the output.
        let neg_zero = first.neg && f == 0;
        let render_at = |k: i128| -> String {
            match &opts.fmt {
                Some(spec) => match exact_at(k) {
                    Some(v) => printf_exact_at(spec, v, sc, neg_zero && k == 0),
                    None => printf_fmt(spec, value_at(&first, &increment, k as f64)),
                },
                None => match exact_at(k) {
                    Some(v) => {
                        let s = render_scaled(v / div, precision.max(0) as usize);
                        if hex_float {
                            generic_format('g', s.parse::<f64>().unwrap_or(0.0), None, false)
                        } else if v == 0 && neg_zero && k == 0 {
                            format!("-{}", s)
                        } else {
                            s
                        }
                    }
                    None => format_float(
                        value_at(&first, &increment, k as f64),
                        precision.max(0) as usize,
                    ),
                },
            }
        };
        let width = if opts.widths {
            field_of(&first, || render_at(0))
                .max(if is_hex_operand(&last.text) {
                    0
                } else {
                    int_field(&render_operand(l, sc, precision))
                })
                .max(field_of(
                    &increment,
                    || render_scaled(i / div, precision.max(0) as usize),
                ))
        } else {
            0
        };
        for k in 0..=steps {
            let mut line = render_at(k);
            if width > 0 {
                line = pad_int(&line, width);
            }
            let last_line = k == steps;
            if write!(stdout, "{}{}", line, if last_line { &term } else { &opts.sep }).is_err() {
                return Ok(0);
            }
            if k % 8192 == 0 {
                let _ = stdout.flush();
            }
        }
        let _ = stdout.flush();
        return Ok(0);
    }

    // exact big-decimal path for literals that do not fit i128
    if scale <= 1_000_000 && !first.value.is_nan() && !last.value.is_nan() && !increment.value.is_nan() {
        if let (Some(f), Some(i), Some(l)) = (
            big_from_dec(&first, scale),
            big_from_dec(&increment, scale),
            big_from_dec(&last, scale),
        ) {
            let render = |v: &Big| -> String {
                match &opts.fmt {
                    Some(spec) => printf_exact(spec, v.neg, &v.d, scale),
                    None => {
                        if hex_float {
                            let s = big_render(v, scale, precision);
                            generic_format('g', s.parse::<f64>().unwrap_or(0.0), None, false)
                        } else {
                            big_render(v, scale, precision)
                        }
                    }
                }
            };
            let width = if opts.widths {
                field_of(&first, || render(&f))
                    .max(field_of(&last, || render(&l)))
                    .max(field_of(&increment, || render(&i)))
            } else {
                0
            };
            let mut v = f;
            let mut prev: Option<String> = None;
            let mut n: u64 = 0;
            loop {
                let past = if i.neg {
                    big_cmp(&v, &l) == std::cmp::Ordering::Less
                } else {
                    big_cmp(&v, &l) == std::cmp::Ordering::Greater
                };
                if past {
                    break;
                }
                let mut line = render(&v);
                if width > 0 {
                    line = pad_int(&line, width);
                }
                if let Some(p) = prev.take() {
                    if write!(stdout, "{}{}", p, opts.sep).is_err() {
                        return Ok(0);
                    }
                }
                prev = Some(line);
                v = big_add(&v, &i);
                if n % 8192 == 0 {
                    let _ = stdout.flush();
                }
                n += 1;
            }
            if let Some(p) = prev {
                let _ = write!(stdout, "{}{}", p, term);
            }
            let _ = stdout.flush();
            return Ok(0);
        }
    }

    // f64 fallback for literals too large to scale into i128
    let width = if opts.widths {
        field_of(&first, || fallback_line(&opts, first.value, precision, hex_float))
            .max(field_of(
                &last,
                || fallback_line(&opts, last.value, precision, hex_float),
            ))
            .max(field_of(
                &increment,
                || fallback_line(&opts, increment.value, precision, hex_float),
            ))
    } else {
        0
    };
    let mut k: u64 = 0;
    let mut prev: Option<String> = None;
    loop {
        let v = value_at(&first, &increment, k as f64);
        if !v.is_finite() {
            break;
        }
        if last.value.is_finite() {
            if down {
                if v < last.value {
                    break;
                }
            } else if v > last.value {
                break;
            }
        }
        let mut line = fallback_line(&opts, v, precision, hex_float);
        if width > 0 {
            line = pad_int(&line, width);
        }
        if let Some(p) = prev {
            if write!(stdout, "{}{}", p, opts.sep).is_err() {
                return Ok(0);
            }
        }
        prev = Some(line);
        if k % 8192 == 0 {
            let _ = stdout.flush();
        }
        k += 1;
    }
    if let Some(p) = prev {
        let _ = write!(stdout, "{}{}", p, term);
    }
    let _ = stdout.flush();
    Ok(0)
}


/// Arbitrary-precision decimal integer: big-endian digits, normalized, signed.
#[derive(Clone)]
struct Big {
    neg: bool,
    d: Vec<u8>,
}

fn big_from_dec(dec: &Dec, scale: i64) -> Option<Big> {
    // The big path works from the decimal literal, so a magnitude that
    // overflows f64 (1e1200) is still exact here; inf/nan literals are not
    // decimal operands and stay in the float path.
    let body = dec.text.trim_start_matches(['-', '+']);
    if !body
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit() || c == '.')
    {
        return None;
    }
    let shift = dec.point + scale - dec.digits.len() as i64;
    if shift < 0 {
        return None;
    }
    let mut d: Vec<u8> = dec.digits.iter().map(|c| c - b'0').collect();
    if shift > 1_000_000 {
        return None;
    }
    for _ in 0..shift {
        d.push(0);
    }
    while d.len() > 1 && d[0] == 0 {
        d.remove(0);
    }
    if d.len() == 1 && d[0] == 0 {
        return Some(Big { neg: dec.neg, d });
    }
    if d.len() > 1_000_000 {
        return None;
    }
    Some(Big { neg: dec.neg, d })
}

fn big_cmp_abs(a: &[u8], b: &[u8]) -> std::cmp::Ordering {
    let na = a.len() - a.iter().take_while(|c| **c == 0).count();
    let nb = b.len() - b.iter().take_while(|c| **c == 0).count();
    if na != nb {
        return na.cmp(&nb);
    }
    let (sa, sb) = (&a[a.len() - na..], &b[b.len() - nb..]);
    for (x, y) in sa.iter().zip(sb.iter()) {
        if x != y {
            return x.cmp(y);
        }
    }
    std::cmp::Ordering::Equal
}

fn big_cmp(a: &Big, b: &Big) -> std::cmp::Ordering {
    let az = big_cmp_abs(&a.d, &[0u8]) == std::cmp::Ordering::Equal;
    let bz = big_cmp_abs(&b.d, &[0u8]) == std::cmp::Ordering::Equal;
    if az && bz {
        return std::cmp::Ordering::Equal;
    }
    if a.neg != b.neg {
        return if a.neg {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        };
    }
    let o = big_cmp_abs(&a.d, &b.d);
    if a.neg {
        o.reverse()
    } else {
        o
    }
}

fn add_abs(a: &[u8], b: &[u8]) -> Vec<u8> {
    let n = a.len().max(b.len());
    let mut out = vec![0u8; n + 1];
    let mut carry = 0u8;
    for i in 0..n {
        let x = a.iter().rev().nth(i).copied().unwrap_or(0);
        let y = b.iter().rev().nth(i).copied().unwrap_or(0);
        let s = x + y + carry;
        out[n - i] = s % 10;
        carry = s / 10;
    }
    out[0] = carry;
    let keep = out.iter().skip_while(|c| **c == 0).count().max(1);
    let start = out.len() - keep;
    out[start..].to_vec()
}

fn sub_abs(a: &[u8], b: &[u8]) -> Vec<u8> {
    // assumes |a| >= |b|
    let n = a.len();
    let mut out = a.to_vec();
    let mut borrow: i32 = 0;
    for i in 0..n {
        let x = out[n - 1 - i] as i32 - borrow;
        let y = b.iter().rev().nth(i).copied().unwrap_or(0) as i32;
        let mut d = x - y;
        if d < 0 {
            d += 10;
            borrow = 1;
        } else {
            borrow = 0;
        }
        out[n - 1 - i] = d as u8;
    }
    let keep = out.iter().skip_while(|c| **c == 0).count().max(1);
    let start = out.len() - keep;
    out[start..].to_vec()
}

fn big_add(a: &Big, b: &Big) -> Big {
    if a.neg == b.neg {
        return Big {
            neg: a.neg,
            d: add_abs(&a.d, &b.d),
        };
    }
    match big_cmp_abs(&a.d, &b.d) {
        std::cmp::Ordering::Equal => Big { neg: false, d: vec![0u8] },
        std::cmp::Ordering::Greater => Big {
            neg: a.neg,
            d: sub_abs(&a.d, &b.d),
        },
        std::cmp::Ordering::Less => Big {
            neg: b.neg,
            d: sub_abs(&b.d, &a.d),
        },
    }
}

/// Render one of the literal operands at the common precision (used for -w).
fn render_operand(v: i128, scale: i64, precision: i64) -> String {
    let drop = (scale - precision).max(0) as u32;
    render_scaled(v / 10i128.pow(drop), precision.max(0) as usize)
}

fn big_render(v: &Big, scale: i64, precision: i64) -> String {
    let p = precision.max(0) as usize;
    let drop = (scale - precision).max(0) as usize;
    let len = v.d.len();
    let head_len = len.saturating_sub(drop);
    let mut head: Vec<u8> = v.d[..head_len].to_vec();
    while head.len() > 1 && head[0] == 0 {
        head.remove(0);
    }
    if head.is_empty() {
        // the whole value rounds away at this precision
        head.push(0);
    }
    let hs: String = head.iter().map(|c| (b'0' + c) as char).collect();
    let mut out = String::new();
    if v.neg {
        out.push('-');
    }
    if p == 0 {
        out.push_str(&hs);
    } else if hs.len() > p {
        out.push_str(&hs[..hs.len() - p]);
        out.push('.');
        out.push_str(&hs[hs.len() - p..]);
    } else {
        out.push('0');
        out.push('.');
        out.push_str(&"0".repeat(p - hs.len()));
        out.push_str(&hs);
    }
    out
}

/// k-th value in the f64 path (`k == 0` keeps FIRST exact, so an infinite
/// increment does not poison it with inf * 0 = NaN).
fn value_at(first: &Dec, increment: &Dec, k: f64) -> f64 {
    if k == 0.0 {
        first.value
    } else {
        first.value + increment.value * k as f64
    }
}


fn fallback_line(opts: &Opts, value: f64, precision: i64, hex_float: bool) -> String {
    match &opts.fmt {
        Some(spec) => printf_fmt(spec, value),
        None => {
            if hex_float {
                generic_format('g', value, None, false)
            } else {
                format_float(value, precision.max(0) as usize)
            }
        }
    }
}

fn cmp_decimal(a: &Dec, b: &Dec) -> i32 {
    let scale = a.decimals().max(b.decimals());
    match (a.scaled(scale), b.scaled(scale)) {
        (Some(x), Some(y)) => x.cmp(&y) as i32,
        _ => a
            .value
            .partial_cmp(&b.value)
            .map_or(0, |o| o as i32),
    }
}

fn format_float(v: f64, precision: usize) -> String {
    if precision > 0 {
        format!("{:.precision$}", v)
    } else if v.fract() == 0.0 && v.abs() < 9e15 {
        format!("{}", v as i64)
    } else {
        format!("{:.0}", v)
    }
}

fn render_scaled(v: i128, precision: usize) -> String {
    let neg = v < 0;
    let a = if neg { -v } else { v };
    let s = a.to_string();
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    if precision == 0 {
        out.push_str(&s);
        return out;
    }
    let padded = if s.len() <= precision {
        format!("{}{}", "0".repeat(precision - s.len() + 1), s)
    } else {
        s
    };
    let split = padded.len() - precision;
    out.push_str(&padded[..split]);
    out.push('.');
    out.push_str(&padded[split..]);
    out
}

/// Width of the integer field of a rendered number, sign included.
fn int_field(s: &str) -> usize {
    let negative = s.starts_with('-');
    let body = if negative { &s[1..] } else { s };
    let digits = match body.find('.') {
        Some(p) => p,
        None => body.len(),
    };
    digits + usize::from(negative)
}

/// Left-pad the integer field with zeros, keeping the sign in front.
fn pad_int(s: &str, width: usize) -> String {
    let cur = int_field(s);
    if cur >= width {
        return s.to_string();
    }
    let fill = "0".repeat(width - cur);
    match s.strip_prefix('-') {
        Some(rest) => format!("-{}{}", fill, rest),
        None => format!("{}{}", fill, s),
    }
}

fn escape_for_display(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Normalise a decimal digit string in place (big-endian, no leading zeros).
fn norm_digits(d: &mut Vec<u8>) {
    match d.iter().position(|&c| c != 0) {
        Some(i) => {
            d.drain(..i);
        }
        None => {
            d.clear();
            d.push(0);
        }
    }
}

fn double_digits(d: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(d.len() + 1);
    let mut carry = 0u8;
    for &c in d.iter().rev() {
        let v = c * 2 + carry;
        out.push(v % 10);
        carry = v / 10;
    }
    if carry > 0 {
        out.push(carry);
    }
    out.reverse();
    norm_digits(&mut out);
    out
}

fn cmp_digits(a: &[u8], b: &[u8]) -> std::cmp::Ordering {
    if a.len() != b.len() {
        return a.len().cmp(&b.len());
    }
    a.cmp(b)
}

/// Round the normalised digit string `m` to `n` significant digits (half-even).
/// Second return value is whether rounding carried into a new leading digit
/// (9.99 -> 10), which shifts the decimal exponent by one.
fn round_sig(m: &[u8], n: usize) -> (Vec<u8>, bool) {
    if n == 0 {
        return (vec![0], false);
    }
    if m.len() <= n {
        let mut out = m.to_vec();
        while out.len() < n {
            out.push(0);
        }
        return (out, false);
    }
    let head = &m[..n];
    let tail = &m[n..];
    let drop = tail.len();
    let twice = double_digits(tail);
    let mut unit = Vec::with_capacity(drop + 1);
    unit.push(1);
    unit.extend(std::iter::repeat(0).take(drop));
    use std::cmp::Ordering;
    let round_up = match cmp_digits(&twice, &unit) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => *head.last().unwrap() % 2 == 1,
    };
    let mut out = head.to_vec();
    let mut carried = false;
    if round_up {
        let mut i = out.len();
        let mut done = false;
        while i > 0 {
            i -= 1;
            if out[i] == 9 {
                out[i] = 0;
            } else {
                out[i] += 1;
                done = true;
                break;
            }
        }
        if !done {
            out.insert(0, 1);
            carried = true;
        }
    }
    (out, carried)
}

/// Scientific form of |value| = digits * 10^-scale: normalised mantissa digits M
/// (no leading or trailing zeros) with the first significant digit at decimal
/// exponent E, i.e. value = M[0].M[1..] e E.
fn sci_form(digits: &[u8], scale: i64) -> (Vec<u8>, i64) {
    let mut m = digits.to_vec();
    norm_digits(&mut m);
    if m.len() == 1 && m[0] == 0 {
        return (vec![0], 0);
    }
    let e = (m.len() as i64) - scale - 1;
    let mut end = m.len();
    while end > 1 && m[end - 1] == 0 {
        end -= 1;
    }
    m.truncate(end);
    (m, e)
}

fn render_exp(m: &[u8], e: i64, frac: usize, strip: bool) -> String {
    let mut md = m.to_vec();
    while md.len() < frac + 1 {
        md.push(0);
    }
    if md.len() > frac + 1 {
        md.truncate(frac + 1);
    }
    if strip && frac > 0 {
        while md.len() > 1 && *md.last().unwrap() == 0 {
            md.pop();
        }
    }
    let head = (b'0' + md[0]) as char;
    let tail: String = md[1..].iter().map(|&c| (b'0' + c) as char).collect();
    let sign = if e < 0 { '-' } else { '+' };
    let edigits = if e < 0 { (-e).to_string() } else { e.to_string() };
    let pad = if edigits.len() < 2 { "0" } else { "" };
    if tail.is_empty() {
        format!("{}e{}{}{}", head, sign, pad, edigits)
    } else {
        format!("{}.{}e{}{}{}", head, tail, sign, pad, edigits)
    }
}

/// Fixed-point rendering of the exact value (mantissa M at exponent E) with
/// `frac` fractional digits, rounding M to E + 1 + frac significant digits.
fn render_fixed(m: &[u8], e: i64, frac: usize) -> String {
    let n = (e + 1 + frac as i64) as usize;
    let (md, carried) = round_sig(m, n);
    let e = if carried { e + 1 } else { e };
    let digits: String = md.iter().map(|&c| (b'0' + c) as char).collect();
    let int_len = e + 1;
    let (int_part, frac_part) = if int_len <= 0 {
        ("0".to_string(), format!("{}{}", "0".repeat((-int_len) as usize), digits))
    } else if (digits.len() as i64) <= int_len {
        (
            format!("{}{}", digits, "0".repeat((int_len - digits.len() as i64) as usize)),
            String::new(),
        )
    } else {
        (
            digits[..(int_len as usize)].to_string(),
            digits[(int_len as usize)..].to_string(),
        )
    };
    let mut frac_part = frac_part;
    if frac_part.len() < frac {
        frac_part.push_str(&"0".repeat(frac - frac_part.len()));
    } else if frac_part.len() > frac {
        frac_part.truncate(frac);
    }
    if frac == 0 {
        int_part
    } else {
        format!("{}.{}", int_part, frac_part)
    }
}

/// Body of a printf conversion applied to an exact decimal value
/// (|value| = digits * 10^-scale).
fn body_exact(conv: char, neg: bool, digits: &[u8], scale: i64, prec: Option<usize>, alt: bool) -> String {
    let p = prec.unwrap_or(6);
    let (m, e) = sci_form(digits, scale);
    let zero = m.len() == 1 && m[0] == 0;
    let body = match conv {
        'e' | 'E' => {
            let (md, carried) = round_sig(&m, p + 1);
            render_exp(&md, if carried { e + 1 } else { e }, p, false)
        }
        'f' | 'F' => {
            let mut s = render_fixed(&m, e, p);
            // `%#.*f` keeps the decimal point even with a zero precision.
            if alt && p == 0 && !s.contains('.') {
                s.push('.');
            }
            s
        }
        _ => {
            let sig = if p == 0 { 1 } else { p };
            if zero {
                let s = render_fixed(&[0u8], 0, sig.saturating_sub(1));
                if alt {
                    s
                } else {
                    strip_frac_zeros(&s)
                }
            } else {
                let (md, carried) = round_sig(&m, sig);
                let e2 = if carried { e + 1 } else { e };
                if e2 < -4 || e2 >= sig as i64 {
                    let mut s = render_exp(&md, e2, sig - 1, !alt);
                    if !alt && s.starts_with(|c: char| c.is_ascii_digit()) {
                        if let Some(i) = s.find(['e', 'E']) {
                            if s[..i].ends_with('.') {
                                s.remove(i - 1);
                            }
                        }
                    }
                    s
                } else {
                    let frac = (sig as i64 - 1 - e2).max(0) as usize;
                    let s = render_fixed_raw(&md, e2, frac);
                    if alt {
                        s
                    } else {
                        strip_frac_zeros(&s)
                    }
                }
            }
        }
    };
    let body = if matches!(conv, 'E' | 'G') {
        body.to_uppercase()
    } else {
        body
    };
    if neg && !body.starts_with('-') {
        let mut s = String::from("-");
        s.push_str(&body);
        return s;
    }
    body
}

/// %g drops trailing zeros only in the fractional part; zeros that are part of
/// the integer field stay (100000 with %.6g prints as 100000, not 1).
fn strip_frac_zeros(s: &str) -> String {
    match s.find('.') {
        Some(dot) => {
            let head = &s[..dot];
            let mut tail = s[dot + 1..].to_string();
            while tail.ends_with('0') {
                tail.pop();
            }
            if tail.is_empty() {
                head.to_string()
            } else {
                format!("{}.{}", head, tail)
            }
        }
        None => s.to_string(),
    }
}

/// Fixed-point rendering from already-rounded mantissa digits.
fn render_fixed_raw(m: &[u8], e: i64, frac: usize) -> String {
    let mut md = m.to_vec();
    let need = (e + 1 + frac as i64) as usize;
    while md.len() < need {
        md.push(0);
    }
    if md.len() > need {
        md.truncate(need);
    }
    render_fixed_exact(&md, e, frac)
}

fn render_fixed_exact(md: &[u8], e: i64, frac: usize) -> String {
    let digits: String = md.iter().map(|&c| (b'0' + c) as char).collect();
    let int_len = e + 1;
    let (int_part, frac_part) = if int_len <= 0 {
        ("0".to_string(), format!("{}{}", "0".repeat((-int_len) as usize), digits))
    } else if (digits.len() as i64) <= int_len {
        (
            format!("{}{}", digits, "0".repeat((int_len - digits.len() as i64) as usize)),
            String::new(),
        )
    } else {
        (
            digits[..(int_len as usize)].to_string(),
            digits[(int_len as usize)..].to_string(),
        )
    };
    let mut frac_part = frac_part;
    if frac_part.len() < frac {
        frac_part.push_str(&"0".repeat(frac - frac_part.len()));
    } else if frac_part.len() > frac {
        frac_part.truncate(frac);
    }
    if frac == 0 {
        int_part
    } else {
        format!("{}.{}", int_part, frac_part)
    }
}

struct Spec {
    left: bool,
    plus: bool,
    space: bool,
    zero: bool,
    alt: bool,
    width: usize,
    prec: Option<usize>,
    conv: char,
}

/// Outcome of scanning one conversion spec.
enum SpecScan {
    Good(Spec),
    /// Recognised printf conversion that is not a floating point one.
    WrongType,
    /// Not a conversion spec at all.
    Invalid,
}

/// Parse one conversion spec; `*i` points just after the leading '%' and is
/// advanced past the conversion character on success.
fn parse_spec_at(chars: &[char], i: &mut usize) -> SpecScan {
    let mut left = false;
    let mut plus = false;
    let mut space = false;
    let mut zero = false;
    let mut alt = false;
    while *i < chars.len() {
        match chars[*i] {
            '-' => left = true,
            '+' => plus = true,
            ' ' => space = true,
            '0' => zero = true,
            '#' => alt = true,
            _ => break,
        }
        *i += 1;
    }
    let mut width: usize = 0;
    while *i < chars.len() && chars[*i].is_ascii_digit() {
        width = width * 10 + (chars[*i] as usize - '0' as usize);
        *i += 1;
    }
    let mut prec: Option<usize> = None;
    if *i < chars.len() && chars[*i] == '.' {
        *i += 1;
        let mut p = 0usize;
        let mut any = false;
        while *i < chars.len() && chars[*i].is_ascii_digit() {
            p = p * 10 + (chars[*i] as usize - '0' as usize);
            any = true;
            *i += 1;
        }
        prec = Some(if any { p } else { 6 });
    }
    while *i < chars.len() && matches!(chars[*i], 'h' | 'l' | 'L' | 'j' | 'z' | 't') {
        *i += 1;
    }
    if *i >= chars.len() {
        return SpecScan::Invalid;
    }
    let conv = chars[*i];
    if matches!(conv, 'e' | 'E' | 'f' | 'F' | 'g' | 'G') {
        *i += 1;
        SpecScan::Good(Spec {
            left,
            plus,
            space,
            zero,
            alt,
            width,
            prec,
            conv,
        })
    } else if matches!(conv, 'd' | 'i' | 'o' | 'u' | 'x' | 'X' | 'c' | 's' | 'b' | 'q' | '*') {
        *i += 1;
        SpecScan::WrongType
    } else {
        SpecScan::Invalid
    }
}

/// Render FORMAT around the value produced by `body` (literal text and `%%` are kept).
fn render_format(spec: &str, body: &mut dyn FnMut(&Spec) -> String) -> String {
    let chars: Vec<char> = spec.chars().collect();
    let mut out = String::new();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] != '%' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        if i + 1 < chars.len() && chars[i + 1] == '%' {
            out.push('%');
            i += 2;
            continue;
        }
        let mut j = i + 1;
        match parse_spec_at(&chars, &mut j) {
            SpecScan::Good(sp) => {
                out.push_str(&body(&sp));
                i = j;
            }
            _ => {
                out.push('%');
                i += 1;
            }
        }
    }
    out
}

/// Check FORMAT the way the reference does (exactly one usable % directive).
fn validate_format(spec: &str) -> Result<(), String> {
    let chars: Vec<char> = spec.chars().collect();
    let mut i = 0usize;
    let mut count = 0usize;
    while i < chars.len() {
        if chars[i] != '%' {
            i += 1;
            continue;
        }
        if i + 1 < chars.len() && chars[i + 1] == '%' {
            i += 2;
            continue;
        }
        if i + 1 >= chars.len() {
            return Err(if count > 0 {
                format!("format '{}' has too many % directives", spec)
            } else {
                format!("format '{}' ends in %", spec)
            });
        }
        let start = i;
        let mut j = i + 1;
        match parse_spec_at(&chars, &mut j) {
            SpecScan::Good(_) => {
                i = j;
                count += 1;
                if count > 1 {
                    return Err(format!("format '{}' has too many % directives", spec));
                }
            }
            SpecScan::WrongType => {
                if count > 0 {
                    return Err(format!("format '{}' has too many % directives", spec));
                }
                return Err("wrong % directive type was given".to_string());
            }
            SpecScan::Invalid => {
                let end = (start + 2).min(chars.len());
                let prefix: String = chars[start..end].iter().collect();
                return Err(format!("{}: invalid conversion specification", prefix));
            }
        }
    }
    if count == 0 {
        return Err(format!("format '{}' has no % directive", spec));
    }
    Ok(())
}

fn apply_flags(body: &str, left: bool, plus: bool, space: bool, zero: bool, width: usize) -> String {
    let mut body = body.to_string();
    if !body.starts_with('-') {
        if plus {
            body.insert(0, '+');
        } else if space {
            body.insert(0, ' ');
        }
    }
    let len = body.chars().count();
    if len >= width {
        return body;
    }
    if left {
        body.push_str(&" ".repeat(width - len));
        return body;
    }
    if zero {
        let pad = "0".repeat(width - len);
        return match body.strip_prefix(['-', '+', ' ']) {
            Some(rest) => {
                let sign = &body[..body.len() - rest.len()];
                format!("{}{}{}", sign, pad, rest)
            }
            None => format!("{}{}", pad, body),
        };
    }
    body.insert_str(0, &" ".repeat(width - len));
    body
}

/// f64-based body of a printf conversion (fallback path only).
fn generic_format(conv: char, value: f64, prec: Option<usize>, alt: bool) -> String {
    let p = prec.unwrap_or(6);
    match conv {
        'e' | 'E' => fix_exponent(&format!("{:.*e}", p, value), matches!(conv, 'E')),
        'f' | 'F' => {
            let mut s = format!("{:.*}", p, value);
            if alt && p == 0 && !s.contains('.') {
                s.push('.');
            }
            s
        }
        _ => {
            let sig = (if p == 0 { 1 } else { p }) as i64;
            let exp = decimal_exponent(value);
            if exp < -4 || exp >= sig {
                fix_exponent(&format!("{:.*e}", (sig - 1) as usize, value), matches!(conv, 'G'))
            } else {
                let dec = (sig - exp - 1).max(0) as usize;
                let mut s = format!("{:.*}", dec, value);
                if !alt {
                    s = s.trim_end_matches('0').to_string();
                    if s.ends_with('.') {
                        s.pop();
                    }
                }
                s
            }
        }
    }
}

/// Exponent of the first significant digit (the C `%g` decision value).
fn decimal_exponent(value: f64) -> i64 {
    if value == 0.0 {
        return 1;
    }
    let s = format!("{:e}", value.abs());
    match s.find('e') {
        Some(i) => s[i + 1..].parse::<i64>().unwrap_or(0),
        None => 0,
    }
}

fn fix_exponent(s: &str, upper: bool) -> String {
    match s.find(['e', 'E']) {
        Some(i) => {
            let mant = &s[..i];
            let exp = &s[i + 1..];
            let (sign, digits) = match exp.strip_prefix('-') {
                Some(rest) => ('-', rest),
                None => ('+', exp.trim_start_matches('+')),
            };
            let padded = if digits.len() < 2 {
                format!("0{}", digits)
            } else {
                digits.to_string()
            };
            let e_char = if upper { 'E' } else { 'e' };
            format!("{}{}{}{}", mant, e_char, sign, padded)
        }
        None => s.to_string(),
    }
}

/// printf subset accepted by -f/--format (f64 value).
fn printf_fmt(spec: &str, value: f64) -> String {
    render_format(spec, &mut |sp| {
        let body = generic_format(sp.conv, value, sp.prec, sp.alt);
        apply_flags(&body, sp.left, sp.plus, sp.space, sp.zero, sp.width)
    })
}

/// printf subset applied to an exact decimal value (|v| = digits * 10^-scale).
fn printf_exact(spec: &str, neg: bool, digits: &[u8], scale: i64) -> String {
    render_format(spec, &mut |sp| {
        let body = body_exact(sp.conv, neg, digits, scale, sp.prec, sp.alt);
        apply_flags(&body, sp.left, sp.plus, sp.space, sp.zero, sp.width)
    })
}

/// printf subset applied to a scaled exact integer value.
fn printf_exact_at(spec: &str, v: i128, scale: i64, neg_zero: bool) -> String {
    let neg = v < 0 || (v == 0 && neg_zero);
    let a = if v < 0 { -v } else { v };
    let digits: Vec<u8> = a.to_string().bytes().map(|c| c - b'0').collect();
    printf_exact(spec, neg, &digits, scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_decimals() {
        assert_eq!(dec_parse("1.5").unwrap().decimals(), 1);
        assert_eq!(dec_parse("1.5e-2").unwrap().decimals(), 3);
        assert_eq!(dec_parse("1e-100").unwrap().decimals(), 100);
        assert_eq!(dec_parse("1e100").unwrap().decimals(), 0);
        assert_eq!(dec_parse(".5").unwrap().decimals(), 1);
        assert_eq!(dec_parse("5.").unwrap().decimals(), 0);
        assert!(dec_parse("1.2.3").is_none());
        assert!(dec_parse("1e").is_none());
        assert!(dec_parse("1e1.5").is_none());
        assert!(dec_parse("1/3").is_none());
        assert!(dec_parse(",").is_none());
        assert_eq!(dec_parse("100").unwrap().scaled(2), Some(10000));
    }

    #[test]
    fn exact_scaling() {
        let f = dec_parse("0").unwrap();
        let i = dec_parse("0.1").unwrap();
        let l = dec_parse("0.3").unwrap();
        assert_eq!(
            (f.scaled(1), i.scaled(1), l.scaled(1)),
            (Some(0), Some(1), Some(3))
        );
        let l2 = dec_parse("0.2999999999999999").unwrap();
        assert_eq!(l2.scaled(16), Some(2999999999999999));
        assert_eq!(dec_parse("1e20").unwrap().scaled(0), Some(100_000_000_000_000_000_000));
    }

    #[test]
    fn padding_and_render() {
        assert_eq!(render_scaled(150, 2), "1.50");
        assert_eq!(render_scaled(-5, 3), "-0.005");
        assert_eq!(render_scaled(7, 0), "7");
        assert_eq!(pad_int("-1.5", 6), "-00001.5");
        assert_eq!(pad_int("1.5", 2), "01.5");
        assert_eq!(pad_int("9", 3), "009");
        assert_eq!(pad_int("08.5", 2), "08.5");
        assert_eq!(int_field("-1.0"), 2);
        assert_eq!(int_field("100.5"), 3);
    }

    #[test]
    fn printf_subset() {
        assert_eq!(printf_fmt("%g", 1.0), "1");
        assert_eq!(printf_fmt("%.3f", 1.5), "1.500");
        assert_eq!(printf_fmt("%e", 1.0), "1.000000e+00");
        assert_eq!(printf_fmt("%5.2f", 1.5), " 1.50");
        assert_eq!(printf_fmt("%08.2f", 1.5), "00001.50");
    }
}
