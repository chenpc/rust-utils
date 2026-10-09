//! 最小正则子集：literal、`.`、`*`、`+`、`?`、`[...]`（含 a-z 範圍）、`|`、`\` 跳脫。
//! 完整 BRE/ERE 需要外部 crate，與「不加依賴」的限制衝突（見 docs/assumptions.md）。

enum Atom {
    Ch(u8),
    Any,
    Set(Vec<u8>),
}

struct Piece {
    atom: Atom,
    min: usize,
    max: Option<usize>,
    group: Option<usize>,
}

pub struct Regex {
    alts: Vec<Vec<Piece>>,
    groups: usize,
}

fn atom_matches(a: &Atom, c: u8) -> bool {
    match a {
        Atom::Ch(x) => *x == c,
        Atom::Any => true,
        Atom::Set(s) => s.contains(&c),
    }
}

fn parse_set(s: &[u8], start: &mut usize) -> Vec<u8> {
    let mut set = Vec::new();
    let mut i = *start;
    while i < s.len() && s[i] != b']' {
        if s[i] == b'\\' && i + 1 < s.len() {
            set.push(s[i + 1]);
            i += 2;
            continue;
        }
        if i + 2 < s.len() && s[i + 1] == b'-' && s[i + 2] != b']' {
            let lo = s[i];
            let hi = s[i + 2];
            let mut c = lo;
            while c <= hi {
                set.push(c);
                c += 1;
            }
            i += 3;
            continue;
        }
        set.push(s[i]);
        i += 1;
    }
    *start = i + 1;
    set
}

pub fn compile(pat: &[u8]) -> Regex {
    let mut alts: Vec<Vec<Piece>> = Vec::new();
    let mut cur: Vec<Piece> = Vec::new();
    let mut groups = 0usize;
    let mut stack: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < pat.len() {
        if pat[i] == b'|' {
            alts.push(std::mem::take(&mut cur));
            i += 1;
            continue;
        }
        let mut group = stack.last().copied();
        if pat[i] == b'\\' && i + 1 < pat.len() && (pat[i + 1] == b'(' || pat[i + 1] == b')') {
            if pat[i + 1] == b'(' {
                groups += 1;
                stack.push(groups);
                group = Some(groups);
            } else {
                stack.pop();
            }
            i += 2;
            continue;
        }
        let atom = if pat[i] == b'.' {
            i += 1;
            Atom::Any
        } else if pat[i] == b'\\' && i + 1 < pat.len() {
            i += 2;
            Atom::Ch(pat[i - 1])
        } else if pat[i] == b'[' {
            let mut start = i + 1;
            let set = parse_set(pat, &mut start);
            i = start;
            Atom::Set(set)
        } else {
            i += 1;
            Atom::Ch(pat[i - 1])
        };
        let (min, max) = if i < pat.len() && pat[i] == b'*' {
            i += 1;
            (0, None)
        } else if i < pat.len() && pat[i] == b'+' {
            i += 1;
            (1, None)
        } else if i < pat.len() && pat[i] == b'?' {
            i += 1;
            (0, Some(1))
        } else {
            (1, Some(1))
        };
        cur.push(Piece { atom, min, max, group });
    }
    alts.push(cur);
    Regex { alts, groups }
}

fn match_pieces(
    pieces: &[Piece],
    s: &[u8],
    pos: usize,
    budget: Option<usize>,
    ranges: &mut Vec<(usize, usize)>,
) -> Option<usize> {
    if pieces.is_empty() {
        return Some(pos);
    }
    let p = &pieces[0];
    let rest = &pieces[1..];
    let idx = ranges.len() - pieces.len();
    let mut n = 0;
    while n < p.min {
        if pos + n >= s.len() || !atom_matches(&p.atom, s[pos + n]) {
            return None;
        }
        n += 1;
    }
    // try each possible repetition count, longest first (leftmost-longest)
    let mut max = n;
    while max < s.len() - pos && (p.max.is_none() || max < p.max.unwrap()) && atom_matches(&p.atom, s[pos + max]) {
        max += 1;
    }
    let mut k = max;
    loop {
        if let Some(end) = match_pieces(rest, s, pos + k, budget, ranges) {
            if budget.is_none() || end <= budget.unwrap() {
                ranges[idx] = (pos, pos + k);
                return Some(end);
            }
        }
        if k == n {
            return None;
        }
        k -= 1;
    }
}

fn group_ranges(pieces: &[Piece], ranges: &[(usize, usize)], n: usize) -> Vec<(usize, usize)> {
    let mut out = vec![(usize::MAX, 0usize); n];
    for (i, p) in pieces.iter().enumerate() {
        if let Some(g) = p.group {
            let (a, b) = ranges[i];
            out[g - 1].0 = out[g - 1].0.min(a);
            out[g - 1].1 = out[g - 1].1.max(b);
        }
    }
    for r in &mut out {
        if r.0 == usize::MAX {
            r.0 = 0;
        }
    }
    out
}

/// 完整字串是否符合（從頭到尾）。
pub fn is_match(re: &Regex, s: &[u8]) -> bool {
    let mut ranges = vec![(0usize, 0usize); re.alts.iter().map(|a| a.len()).max().unwrap_or(0)];
    for alt in &re.alts {
        for r in ranges.iter_mut() {
            *r = (0, 0);
        }
        if match_pieces(alt, s, 0, Some(s.len()), &mut ranges) == Some(s.len()) {
            return true;
        }
    }
    false
}

/// 從頭開始的完整符合，回傳 (start, end, 各 capture group 的 range)。
pub fn match_with_groups(re: &Regex, s: &[u8]) -> Option<(usize, usize, Vec<(usize, usize)>)> {
    for alt in &re.alts {
        let mut ranges = vec![(0usize, 0usize); alt.len()];
        if let Some(end) = match_pieces(alt, s, 0, Some(s.len()), &mut ranges) {
            if end == s.len() {
                return Some((0, end, group_ranges(alt, &ranges, re.groups)));
            }
        }
    }
    None
}

/// 最左邊的符合位置與長度。
pub fn find(re: &Regex, s: &[u8]) -> Option<(usize, usize)> {
    for start in 0..=s.len() {
        for alt in &re.alts {
            let mut ranges = vec![(0usize, 0usize); alt.len()];
            if let Some(end) = match_pieces(alt, s, start, None, &mut ranges) {
                if end > start {
                    return Some((start, end));
                }
            }
        }
    }
    None
}
