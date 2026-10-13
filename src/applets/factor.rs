//! factor — 印出 NUMBER 的質因數。
//!
//! 規格來源：`docs/man/factor.1.gz`（`-h, --exponents  Print factors in the form p^e`、
//! `[NUMBER]`；「Print the prime factors of the given NUMBER(s). If none are specified,
//! read from standard input.」）。
//!
//! 因數分解採自寫實作（不引入外部依賴）：
//! - 小質數試除
//! - `n < 2^64`：原生 u64 Miller–Rabin（決定性 base 集）＋ Pollard–Brent（u128 乘法）
//! - 更大的 n：自寫任意精度（基 2^32）＋ Miller–Rabin ＋ Pollard–Brent，
//!   modmul 以「加倍法」實作（只需加法與移位，避免多精度長除法；長除僅用於 n/d 還原）

use crate::pathname;
use crate::{In, Out};

const FACTOR_HELP: &str = r#"Print the prime factors of the given NUMBER(s).
If none are specified, read from standard input.

Usage: factor [OPTION]... [NUMBER]...

Arguments:
  [NUMBER]...  

Options:
  -h, --exponents  Print factors in the form p^e
      --help       Print help information.
  -V, --version    Print version
"#;

const FACTOR_USAGE: &str = "factor [OPTION]... [NUMBER]...";
const FACTOR_LONGS: &[&str] = &["exponents", "help", "version"];
const BASE: u64 = 1 << 32;

struct Rng {
    state: u64,
}

impl Rng {
    fn next(&mut self) -> u64 {
        // SplitMix64
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, limit: u64) -> u64 {
        if limit == 0 {
            return 0;
        }
        self.next() % limit
    }
}

const SMALL_PRIMES: &[u64] = &[
    2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89,
    97, 101, 103, 107, 109, 113, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173, 179, 181, 191,
    193, 197, 199, 211, 223, 227, 229, 233, 239, 241, 251, 257, 263, 269, 277, 281, 283, 293, 307,
    311, 313, 317, 331, 337, 347, 349, 353, 359, 367, 373, 379, 383, 389, 397, 401, 409, 419, 421,
    431, 433, 439, 443, 449, 457, 461, 463, 467, 479, 487, 491, 499, 503, 509, 521, 523, 541, 547,
    557, 563, 569, 571, 577, 587, 593, 599, 601, 607, 613, 617, 619, 631, 641, 643, 647, 653, 659,
    661, 673, 677, 683, 691, 701, 709, 719, 727, 733, 739, 743, 751, 757, 761, 769, 773, 787, 797,
    809, 811, 821, 823, 827, 829, 839, 853, 857, 859, 863, 877, 881, 883, 887, 907, 911, 919, 929,
    937, 941, 947, 953, 967, 971, 977, 983, 991, 997,
];

// ---------------------------------------------------------------- u64 快速路徑

fn mul_mod64(a: u64, b: u64, n: u64) -> u64 {
    ((a as u128 * b as u128) % n as u128) as u64
}

fn pow_mod64(mut base: u64, mut exp: u64, n: u64) -> u64 {
    let mut result = 1u64;
    base %= n;
    while exp > 0 {
        if exp & 1 == 1 {
            result = mul_mod64(result, base, n);
        }
        base = mul_mod64(base, base, n);
        exp >>= 1;
    }
    result
}

fn is_prime64(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    for &p in SMALL_PRIMES {
        if n == p {
            return true;
        }
        if n % p == 0 {
            return false;
        }
    }
    let mut d = n - 1;
    let mut r = 0u32;
    while d & 1 == 0 {
        d >>= 1;
        r += 1;
    }
    for &a in &[2u64, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] {
        if a >= n {
            continue;
        }
        let mut x = pow_mod64(a, d, n);
        if x == 1 || x == n - 1 {
            continue;
        }
        let mut composite = true;
        for _ in 1..r {
            x = mul_mod64(x, x, n);
            if x == n - 1 {
                composite = false;
                break;
            }
        }
        if composite {
            return false;
        }
    }
    true
}

fn pollard64(n: u64, rng: &mut Rng) -> u64 {
    if n % 2 == 0 {
        return 2;
    }
    for _ in 0..32 {
        let c = rng.below(n.saturating_sub(1)) + 1;
        let mut y = rng.below(n.saturating_sub(1)) + 1;
        let mut r: u64 = 1;
        let m: u64 = 64;
        let mut q = 1u64;
        let mut g = 1u64;
        let mut ys = 0u64;
        let mut x = 0u64;
        while g == 1 {
            x = y;
            for _ in 0..r {
                y = (mul_mod64(y, y, n) + c) % n;
            }
            let mut k = 0u64;
            while k < r && g == 1 {
                ys = y;
                let lim = m.min(r - k);
                for _ in 0..lim {
                    y = (mul_mod64(y, y, n) + c) % n;
                    let diff = if x > y { x - y } else { y - x };
                    if diff == 0 {
                        q = 0;
                    } else {
                        q = mul_mod64(q, diff, n);
                    }
                }
                g = gcd64(q, n);
                k += m;
            }
            r *= 2;
            if r > 1 << 20 {
                break;
            }
        }
        if g == n {
            let mut y2 = ys;
            loop {
                y2 = (mul_mod64(y2, y2, n) + c) % n;
                let diff = if x > y2 { x - y2 } else { y2 - x };
                let g2 = gcd64(diff, n);
                if g2 > 1 {
                    if g2 < n {
                        return g2;
                    }
                    break;
                }
            }
            continue;
        }
        if g > 1 {
            return g;
        }
    }
    n
}

fn gcd64(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

fn factor_u64(n: u64, out: &mut Vec<u64>, rng: &mut Rng) {
    let mut cur = n;
    for &p in SMALL_PRIMES {
        if cur % p == 0 {
            while cur % p == 0 {
                cur /= p;
                out.push(p);
            }
        }
    }
    if cur == 1 {
        return;
    }
    factor_rec64(cur, out, rng);
}

fn factor_rec64(n: u64, out: &mut Vec<u64>, rng: &mut Rng) {
    if n == 1 {
        return;
    }
    if is_prime64(n) {
        out.push(n);
        return;
    }
    let d = pollard64(n, rng);
    if d == n {
        out.push(n);
        return;
    }
    factor_rec64(d, out, rng);
    factor_rec64(n / d, out, rng);
}

// -------------------------------------------------------- 任意精度（基 2^32）

#[derive(Clone, PartialEq, Eq)]
struct BigU {
    limbs: Vec<u32>,
}

impl BigU {
    fn from_u64(v: u64) -> BigU {
        BigU {
            limbs: vec![v as u32, (v >> 32) as u32],
        }
        .trim()
    }
    fn zero() -> BigU {
        BigU { limbs: Vec::new() }
    }
    fn trim(mut self) -> BigU {
        while self.limbs.last() == Some(&0) {
            self.limbs.pop();
        }
        self
    }
    fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }
    fn fits_u64(&self) -> bool {
        self.limbs.len() <= 2
    }
    fn to_u64(&self) -> u64 {
        let lo = *self.limbs.get(0).unwrap_or(&0) as u64;
        let hi = *self.limbs.get(1).unwrap_or(&0) as u64;
        lo | (hi << 32)
    }
    fn is_even(&self) -> bool {
        self.limbs.first().map(|l| l & 1 == 0).unwrap_or(true)
    }
    fn bit_len(&self) -> usize {
        match self.limbs.last() {
            None => 0,
            Some(top) => (self.limbs.len() - 1) * 32 + (32 - top.leading_zeros() as usize),
        }
    }
    fn bit(&self, i: usize) -> bool {
        let limb = i / 32;
        limb < self.limbs.len() && (self.limbs[limb] >> (i % 32)) & 1 == 1
    }
    fn cmp(&self, other: &BigU) -> std::cmp::Ordering {
        if self.limbs.len() != other.limbs.len() {
            return self.limbs.len().cmp(&other.limbs.len());
        }
        for i in (0..self.limbs.len()).rev() {
            if self.limbs[i] != other.limbs[i] {
                return self.limbs[i].cmp(&other.limbs[i]);
            }
        }
        std::cmp::Ordering::Equal
    }
    fn add(&self, other: &BigU) -> BigU {
        let n = self.limbs.len().max(other.limbs.len());
        let mut out = Vec::with_capacity(n + 1);
        let mut carry: u64 = 0;
        for i in 0..n {
            let a = *self.limbs.get(i).unwrap_or(&0) as u64;
            let b = *other.limbs.get(i).unwrap_or(&0) as u64;
            let s = a + b + carry;
            out.push(s as u32);
            carry = s >> 32;
        }
        if carry != 0 {
            out.push(carry as u32);
        }
        BigU { limbs: out }.trim()
    }
    /// 假設 self >= other。
    fn sub(&self, other: &BigU) -> BigU {
        let mut out = Vec::with_capacity(self.limbs.len());
        let mut borrow: u64 = 0;
        for i in 0..self.limbs.len() {
            let a = self.limbs[i] as u64;
            let b = *other.limbs.get(i).unwrap_or(&0) as u64;
            if a >= b + borrow {
                out.push((a - b - borrow) as u32);
                borrow = 0;
            } else {
                out.push(((a as u128) + (1u128 << 32) - (b as u128) - (borrow as u128)) as u32);
                borrow = 1;
            }
        }
        BigU { limbs: out }.trim()
    }
    fn shr1(&self) -> BigU {
        let mut out = Vec::with_capacity(self.limbs.len());
        for i in 0..self.limbs.len() {
            let low = self.limbs[i] >> 1;
            let high = self.limbs.get(i + 1).map(|n| n << 31).unwrap_or(0);
            out.push(low | high);
        }
        BigU { limbs: out }.trim()
    }
    fn mod_small(&self, p: u64) -> u64 {
        let mut rem: u128 = 0;
        for i in (0..self.limbs.len()).rev() {
            rem = ((rem << 32) | self.limbs[i] as u128) % p as u128;
        }
        rem as u64
    }
    fn div_small(&self, p: u64) -> BigU {
        let mut out = vec![0u32; self.limbs.len()];
        let mut rem: u128 = 0;
        for i in (0..self.limbs.len()).rev() {
            let cur = (rem << 32) | self.limbs[i] as u128;
            out[i] = (cur / p as u128) as u32;
            rem = cur % p as u128;
        }
        BigU { limbs: out }.trim()
    }
    fn to_string(&self) -> String {
        if self.is_zero() {
            return "0".to_string();
        }
        let mut groups: Vec<u32> = Vec::new();
        let mut cur = self.clone();
        while !cur.is_zero() {
            let mut rem: u128 = 0;
            let mut q = vec![0u32; cur.limbs.len()];
            for i in (0..cur.limbs.len()).rev() {
                let v = (rem << 32) | cur.limbs[i] as u128;
                q[i] = (v / 1_000_000_000) as u32;
                rem = v % 1_000_000_000;
            }
            groups.push(rem as u32);
            cur = BigU { limbs: q }.trim();
        }
        let mut s = groups.pop().unwrap().to_string();
        while let Some(g) = groups.pop() {
            s.push_str(&format!("{:09}", g));
        }
        s
    }
    fn from_dec(s: &str) -> Option<BigU> {
        if s.is_empty() || !s.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let mut n = BigU::zero();
        for c in s.bytes() {
            n = mul_small(&n, 10).add(&BigU::from_u64((c - b'0') as u64));
        }
        Some(n.trim())
    }
}

fn mul_small(n: &BigU, m: u64) -> BigU {
    let mut out = Vec::with_capacity(n.limbs.len() + 2);
    let mut carry: u128 = 0;
    for &l in &n.limbs {
        let v = (l as u128) * (m as u128) + carry;
        out.push(v as u32);
        carry = v >> 32;
    }
    while carry != 0 {
        out.push(carry as u32);
        carry >>= 32;
    }
    BigU { limbs: out }.trim()
}

fn add_mod(a: &BigU, b: &BigU, m: &BigU) -> BigU {
    let s = a.add(b);
    if s.cmp(m) == std::cmp::Ordering::Less {
        s
    } else {
        s.sub(m)
    }
}

/// (a * b) mod n（呼叫端保證 a < n、b < n）。
/// 做法：schoolbook 完整乘積 ＋ Knuth Algorithm D 長除取餘（u128 累計，
/// 借位用「debt」慣例：`debt = -(d >> 32) ≥ 0`）。
/// 成本約 l² 次 u128 乘法（l = n 的肢數），比「逐位加倍」少一個數量級。
fn mul_mod(a: &BigU, b: &BigU, n: &BigU) -> BigU {
    if a.is_zero() || b.is_zero() {
        return BigU::zero();
    }
    if a.fits_u64() && b.fits_u64() && n.fits_u64() {
        return BigU::from_u64(mul_mod64(a.to_u64(), b.to_u64(), n.to_u64()));
    }
    let l = n.limbs.len();
    if l == 1 {
        let m = n.limbs[0] as u128;
        let mut acc: u128 = 0;
        for &limb in &a.limbs {
            acc = ((acc << 32) | limb as u128) % m;
        }
        for &limb in &b.limbs {
            acc = ((acc << 32) | limb as u128) % m;
        }
        return BigU::from_u64(acc as u64);
    }

    // 1) p = a * b（≤ 2l 肢，多留一格當視窗顶端）
    let mut p = vec![0u32; 2 * l + 1];
    for (i, &al) in a.limbs.iter().enumerate() {
        let mut carry: u64 = 0;
        for (j, &bl) in b.limbs.iter().enumerate() {
            let cur = p[i + j] as u64 + (al as u64) * (bl as u64) + carry;
            p[i + j] = cur as u32;
            carry = cur >> 32;
        }
        let mut k = i + b.limbs.len();
        while carry != 0 {
            let cur = p[k] as u64 + carry;
            p[k] = cur as u32;
            carry = cur >> 32;
            k += 1;
        }
    }

    // 2) 正規化：左移 s 位讓 v 的最高肢 ≥ 2^31（Algorithm D 保證 q̂ 誤差 ≤ 2）。
    let s: u32 = n.limbs[l - 1].leading_zeros();
    let mut v = vec![0u32; l];
    let mut carry: u32 = 0;
    for i in 0..l {
        let x = ((n.limbs[i] as u64) << s) | (carry as u64);
        v[i] = x as u32;
        carry = (x >> 32) as u32;
    }
    if s != 0 {
        let mut topacc: u32 = 0;
        for i in 0..p.len() {
            let x = ((p[i] as u64) << s) | (topacc as u64);
            p[i] = x as u32;
            topacc = (x >> 32) as u32;
        }
        debug_assert_eq!(topacc, 0);
    }
    let vtop = v[l - 1] as u128;

    // 3) 長除：由高位往下逐肢估商、減法；只保留餘數。
    for off in (0..=l).rev() {
        let w1 = p[off + l] as u128;
        let w2 = p[off + l - 1] as u128;
        let (mut qhat, mut rhat) = if w1 >= vtop {
            ((BASE - 1) as u128, (BASE - 1) as u128)
        } else {
            let w = (w1 << 32) | w2;
            (w / vtop, w % vtop)
        };
        if l >= 2 && rhat < (BASE as u128) {
            loop {
                let lhs = qhat * (v[l - 2] as u128);
                let rhs = (rhat << 32) | (p[off + l - 2] as u128);
                if lhs <= rhs {
                    break;
                }
                qhat -= 1;
                rhat += vtop;
                if rhat >= (BASE as u128) {
                    break;
                }
            }
        }
        let mut debt: u128 = 0;
        for k in 0..l {
            let d = (p[off + k] as i128) - (v[k] as u128 * qhat) as i128 - (debt as i128);
            p[off + k] = (d & 0xFFFF_FFFF) as u32;
            debt = (-(d >> 32)) as u128;
        }
        let mut top = (p[off + l] as i128) - (debt as i128);
        // q̂ 過大 → 回補 v（Knuth 證明最多兩次）。
        while top < 0 {
            let mut c: i128 = 0;
            for k in 0..l {
                let x = (p[off + k] as i128) + (v[k] as i128) + c;
                p[off + k] = (x & 0xFFFF_FFFF) as u32;
                c = x >> 32;
            }
            top += c;
        }
        p[off + l] = if top == 0 { 0 } else { (top & 0xFFFF_FFFF) as u32 };
    }

    let mut rem = vec![0u32; l];
    rem.copy_from_slice(&p[..l]);
    if s != 0 {
        for i in 0..l - 1 {
            rem[i] = (rem[i] >> s) | (rem[i + 1] << (32 - s));
        }
        rem[l - 1] >>= s;
    }
    // 4) 正規化餘數可能等於 n（或因 q̂ 低估而 ≥ n）→ 條件性減 n。
    loop {
        let mut debt: u128 = 0;
        let mut cand = rem.clone();
        for j in 0..l {
            let d = (cand[j] as i128) - (n.limbs[j] as i128) - (debt as i128);
            cand[j] = (d & 0xFFFF_FFFF) as u32;
            debt = (-(d >> 32)) as u128;
        }
        if debt != 0 {
            break; // cand < n（借了位外）→ 維持 rem 並結束
        }
        rem = cand;
    }
    BigU { limbs: rem }.trim()
}

fn pow_mod(base: &BigU, exp: &BigU, n: &BigU) -> BigU {
    let mut result = BigU::from_u64(1);
    let mut b = base.clone();
    for i in 0..exp.bit_len() {
        if exp.bit(i) {
            result = mul_mod(&result, &b, n);
        }
        b = mul_mod(&b, &b, n);
    }
    result
}

fn gcd_big(mut a: BigU, mut b: BigU) -> BigU {
    if a.is_zero() {
        return b;
    }
    if b.is_zero() {
        return a;
    }
    let mut shift = 0usize;
    while a.is_even() && b.is_even() {
        a = a.shr1();
        b = b.shr1();
        shift += 1;
    }
    while a.is_even() {
        a = a.shr1();
    }
    loop {
        while b.is_even() {
            b = b.shr1();
        }
        if a.cmp(&b) == std::cmp::Ordering::Greater {
            std::mem::swap(&mut a, &mut b);
        }
        b = b.sub(&a);
        if b.is_zero() {
            break;
        }
    }
    for _ in 0..shift {
        a = a.add(&a);
    }
    a
}

fn is_prime_big(n: &BigU) -> bool {
    if n.cmp(&BigU::from_u64(2)) == std::cmp::Ordering::Less {
        return false;
    }
    for &p in SMALL_PRIMES {
        if n.cmp(&BigU::from_u64(p)) == std::cmp::Ordering::Equal {
            return true;
        }
        if n.mod_small(p) == 0 {
            return false;
        }
    }
    let one = BigU::from_u64(1);
    let nm1 = n.sub(&one);
    let mut d = nm1.clone();
    let mut r = 0usize;
    while d.is_even() {
        d = d.shr1();
        r += 1;
    }
    for b in [2u64, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] {
        let bb = BigU::from_u64(b);
        if bb.cmp(n) != std::cmp::Ordering::Less {
            continue;
        }
        let mut x = pow_mod(&bb, &d, n);
        if x.cmp(&one) == std::cmp::Ordering::Equal || x.cmp(&nm1) == std::cmp::Ordering::Equal {
            continue;
        }
        let mut probably_prime = false;
        for _ in 1..r {
            x = mul_mod(&x, &x, n);
            if x.cmp(&nm1) == std::cmp::Ordering::Equal {
                probably_prime = true;
                break;
            }
        }
        if !probably_prime {
            return false;
        }
    }
    true
}

fn random_below_big(limit: &BigU, rng: &mut Rng) -> BigU {
    let limbs = limit.limbs.len().max(1);
    let mut n = BigU::zero();
    for _ in 0..limbs {
        n = mul_small(&n, 1 << 16).add(&BigU::from_u64(rng.next() & 0xFFFF));
    }
    while n.cmp(limit) != std::cmp::Ordering::Less {
        n = n.sub(limit);
    }
    n
}

fn pollard_big(n: &BigU, rng: &mut Rng) -> BigU {
    let one = BigU::from_u64(1);
    for _ in 0..16 {
        let c = random_below_big(&n.sub(&one), rng).add(&one);
        let mut y = random_below_big(&n.sub(&one), rng).add(&one);
        let mut q;
        let mut g = BigU::from_u64(1);
        let mut r: u64 = 1;
        let m: u64 = 32;
        while g.cmp(&one) == std::cmp::Ordering::Equal {
            let x = y.clone();
            for _ in 0..r {
                y = add_mod(&mul_mod(&y, &y, n), &c, n);
            }
            let mut k: u64 = 0;
            while k < r && g.cmp(&one) == std::cmp::Ordering::Equal {
                // 每個批次重新累計 q：q 變成 0 (mod n) 會讓之後的 gcd 永遠是 n。
                q = BigU::from_u64(1);
                let lim = m.min(r - k);
                for _ in 0..lim {
                    y = add_mod(&mul_mod(&y, &y, n), &c, n);
                    let diff = if x.cmp(&y) == std::cmp::Ordering::Greater {
                        x.sub(&y)
                    } else {
                        y.sub(&x)
                    };
                    q = if diff.is_zero() {
                        BigU::zero()
                    } else {
                        mul_mod(&q, &diff, n)
                    };
                }
                g = gcd_big(q.clone(), n.clone());
                k += m;
            }
            r *= 2;
            if r > 1 << 18 {
                break;
            }
        }
        if g.cmp(&one) == std::cmp::Ordering::Greater && g.cmp(n) == std::cmp::Ordering::Less {
            return g;
        }
    }
    n.clone()
}

/// n / d（d 整除 n）：逐肢長除，商肢以二分搜尋求得。
fn div_big(n: &BigU, d: &BigU) -> BigU {
    if d.limbs.len() == 1 {
        return n.div_small(d.limbs[0] as u64);
    }
    let mut q = vec![0u32; n.limbs.len() + 1];
    let mut rem = BigU::zero();
    for i in (0..n.limbs.len()).rev() {
        rem = mul_small(&rem, BASE).add(&BigU::from_u64(n.limbs[i] as u64));
        let mut lo: u64 = 0;
        let mut hi: u64 = u32::MAX as u64;
        while lo < hi {
            let mid = (lo + hi + 1) / 2;
            let prod = mul_small(d, mid);
            if prod.cmp(&rem) == std::cmp::Ordering::Greater {
                hi = mid - 1;
            } else {
                lo = mid;
            }
        }
        q[i] = lo as u32;
        rem = rem.sub(&mul_small(d, lo));
    }
    BigU { limbs: q }.trim()
}

fn factor_big(n: &BigU, out: &mut Vec<BigU>, rng: &mut Rng) {
    let mut cur = n.clone();
    for &p in SMALL_PRIMES {
        if cur.mod_small(p) == 0 {
            while cur.mod_small(p) == 0 {
                cur = cur.div_small(p);
                out.push(BigU::from_u64(p));
            }
        }
    }
    if cur.is_zero() || cur.cmp(&BigU::from_u64(1)) == std::cmp::Ordering::Equal {
        return;
    }
    if cur.fits_u64() {
        let mut small: Vec<u64> = Vec::new();
        factor_rec64(cur.to_u64(), &mut small, rng);
        for v in small {
            out.push(BigU::from_u64(v));
        }
        return;
    }
    factor_rec_big(&cur, out, rng);
}

fn factor_rec_big(n: &BigU, out: &mut Vec<BigU>, rng: &mut Rng) {
    if n.cmp(&BigU::from_u64(1)) != std::cmp::Ordering::Greater {
        return;
    }
    if n.fits_u64() {
        let mut small: Vec<u64> = Vec::new();
        factor_rec64(n.to_u64(), &mut small, rng);
        for v in small {
            out.push(BigU::from_u64(v));
        }
        return;
    }
    if is_prime_big(n) {
        out.push(n.clone());
        return;
    }
    let d = pollard_big(n, rng);
    if d.cmp(n) != std::cmp::Ordering::Less || d.cmp(&BigU::from_u64(1)) != std::cmp::Ordering::Greater {
        // 分解失敗：把自身當因子輸出（參考對超大數同樣可能停滯）。
        out.push(n.clone());
        return;
    }
    factor_rec_big(&d, out, rng);
    let q = div_big(n, &d);
    factor_rec_big(&q, out, rng);
}

fn unexpected(arg: &str, similar: Option<&str>, stderr: Out) -> i32 {
    write!(stderr, "error: unexpected argument '{}' found\n\n", arg).ok();
    if let Some(c) = similar {
        writeln!(stderr, "tip: a similar argument exists: '--{}'", c).ok();
    } else {
        writeln!(
            stderr,
            "  tip: to pass '{}' as a value, use '-- {}'",
            arg, arg
        )
        .ok();
    }
    write!(
        stderr,
        "\nUsage: {}\n\nFor more information, try '--help'.\n",
        FACTOR_USAGE
    )
    .ok();
    1
}

fn invalid_token(token: &str, stderr: Out) {
    writeln!(
        stderr,
        "factor: '{}' is not a valid positive integer",
        token
    )
    .ok();
}

pub fn run(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut exponents = false;
    let mut numbers: Vec<String> = Vec::new();
    let mut only_positional = false;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        i += 1;
        if only_positional {
            numbers.push(a.clone());
            continue;
        }
        if a == "--" {
            only_positional = true;
            continue;
        }
        if a.starts_with("--") {
            let name = a[2..].split('=').next().unwrap_or("").to_string();
            let value = a.split_once('=').map(|(_, v)| v.to_string());
            match pathname::resolve_long(&name, FACTOR_LONGS) {
                Ok("help") => match value {
                    Some(v) => return pathname::clap_unexpected_value("help", &v, FACTOR_USAGE, stderr),
                    None => {
                        write!(stdout, "{}", FACTOR_HELP).ok();
                        return 0;
                    }
                },
                Ok("version") => match value {
                    Some(v) => return pathname::clap_unexpected_value("version", &v, FACTOR_USAGE, stderr),
                    None => {
                        writeln!(stdout, "factor (uutils coreutils) 0.10.0").ok();
                        return 0;
                    }
                },
                Ok("exponents") => match value {
                    Some(v) => return pathname::clap_unexpected_value("exponents", &v, FACTOR_USAGE, stderr),
                    None => exponents = true,
                },
                Ok(other) => return unexpected(&format!("--{}", other), None, stderr),
                Err(similar) => return unexpected(a, similar, stderr),
            }
            continue;
        }
        if a.starts_with('-') && a.len() > 1 {
            let mut k = 1;
            while k < a.len() {
                match a.as_bytes()[k] {
                    b'h' => exponents = true,
                    b'V' => {
                        writeln!(stdout, "factor (uutils coreutils) 0.10.0").ok();
                        return 0;
                    }
                    _ => return unexpected(&a[..=k], None, stderr),
                }
                k += 1;
            }
            continue;
        }
        numbers.push(a.clone());
    }

    let tokens: Vec<String> = if numbers.is_empty() {
        let mut buf = String::new();
        let _ = stdin.read_to_string(&mut buf);
        buf.split_ascii_whitespace().map(|s| s.to_string()).collect()
    } else {
        numbers
    };

    let mut rc = 0;
    let mut rng = Rng { state: 0x243F6A8885A308D3 };
    let one = BigU::from_u64(1);
    for token in tokens {
        let body = token.strip_prefix('+').unwrap_or(&token);
        let n = match BigU::from_dec(body) {
            Some(n) => n,
            None => {
                invalid_token(&token, stderr);
                rc = 1;
                continue;
            }
        };
        write!(stdout, "{}:", n.to_string()).ok();
        if n.cmp(&one) == std::cmp::Ordering::Greater {
            let text: Vec<String>;
            if n.fits_u64() {
                let mut factors: Vec<u64> = Vec::new();
                factor_u64(n.to_u64(), &mut factors, &mut rng);
                factors.sort();
                text = factors.iter().map(|f| f.to_string()).collect();
            } else {
                let mut factors: Vec<BigU> = Vec::new();
                factor_big(&n, &mut factors, &mut rng);
                factors.sort_by(|a, b| a.cmp(b));
                text = factors.iter().map(|f| f.to_string()).collect();
            }
            if exponents {
                let mut idx = 0;
                while idx < text.len() {
                    let mut j = idx + 1;
                    while j < text.len() && text[j] == text[idx] {
                        j += 1;
                    }
                    let count = j - idx;
                    if count > 1 {
                        write!(stdout, " {}^{}", text[idx], count).ok();
                    } else {
                        write!(stdout, " {}", text[idx]).ok();
                    }
                    idx = j;
                }
            } else {
                for f in &text {
                    write!(stdout, " {}", f).ok();
                }
            }
        }
        writeln!(stdout).ok();
    }
    rc
}

#[cfg(test)]
mod arith_tests {
    use super::*;

    fn to128(x: &BigU) -> u128 {
        let mut v: u128 = 0;
        for &l in x.limbs.iter().rev() {
            v = (v << 32) | l as u128;
        }
        v
    }
    fn from128(mut v: u128) -> BigU {
        let mut limbs = Vec::new();
        while v != 0 {
            limbs.push((v & 0xFFFF_FFFF) as u32);
            v >>= 32;
        }
        BigU { limbs }.trim()
    }
    fn dec(s: &str) -> BigU {
        BigU::from_dec(s).unwrap()
    }

    const CASES: &[(&str, &str, &str, &str)] = &[
        // l = 2
        ("1099511627776", "1099511627776", "1099511627783", "49"),
        ("1099511627782", "1099511627782", "1099511627783", "1"),
        ("1099511627782", "2", "1099511627783", "1099511627781"),
        ("1099511627782", "1", "1099511627783", "1099511627782"),
        // l = 3
        ("9223372036854775808", "9223372036854775808", "18446744073709551617", "13835058055282163713"),
        ("274177", "67280421310721", "18446744073709551617", "0"),
        ("1180591620717411303422", "1180591620717411303422", "1180591620717411303427", "25"),
        ("590295810358705651712", "590295810358705651712", "1180591620717411303427", "295147905179352825859"),
        // l = 5
        ("999999999999999999999999", "999999999999999999999999", "1000000000000000000000007", "64"),
    ];

    #[test]
    fn mul_mod_known_cases() {
        for (a, b, n, want) in CASES {
            let (a, b, n) = (dec(a), dec(b), dec(n));
            let got = mul_mod(&a, &b, &n);
            assert_eq!(got.to_string(), *want, "n={} a={} b={}", n.to_string(), a.to_string(), b.to_string());
        }
    }

    #[test]
    fn mul_mod_matches_u128() {
        let mut rng = Rng { state: 20261105 };
        for _ in 0..3000 {
            let bits = 33 + (rng.next() % 28); // n < 2^61 → l = 2
            let n = from128((1u128 << bits) | ((rng.next() as u128) & ((1u128 << bits) - 1)));
            let n128 = to128(&n);
            let a = from128((rng.next() as u128) % n128);
            let b = from128((rng.next() as u128) % n128);
            let want = (to128(&a) * to128(&b)) % n128;
            let got = to128(&mul_mod(&a, &b, &n));
            assert!(got < n128, "not reduced: n={} got={}", n128, got);
            assert_eq!(got, want, "n={} a={} b={}", n128, to128(&a), to128(&b));
        }
    }

    #[test]
    fn mul_mod_matches_u128_three_limbs() {
        let mut rng = Rng { state: 20261108 };
        for _ in 0..3000 {
            // n 落在 [2^64, 2^64 + 2^24) → l = 3；a、b < 2^63 → 乘積 < 2^127 可用 u128 驗證
            let n128 = (1u128 << 64) | (rng.next() as u128 & ((1u128 << 24) - 1));
            let n = from128(n128);
            let a = from128((rng.next() as u128 >> 1) % n128);
            let b = from128((rng.next() as u128 >> 1) % n128);
            let want = (to128(&a) * to128(&b)) % n128;
            let got = to128(&mul_mod(&a, &b, &n));
            assert!(got < n128, "not reduced: n={} got={}", n128, got);
            assert_eq!(got, want, "n={} a={} b={}", n128, to128(&a), to128(&b));
        }
    }

    #[test]
    fn pow_mod_matches_u128() {
        let mut rng = Rng { state: 20261106 };
        for _ in 0..400 {
            let bits = 33 + (rng.next() % 28);
            let n = from128((1u128 << bits) | ((rng.next() as u128) & ((1u128 << bits) - 1)));
            let n128 = to128(&n);
            let base = from128((rng.next() as u128) % n128);
            let exp = from128((rng.next() as u128) % n128);
            let mut want: u128 = 1;
            let mut b = to128(&base) % n128;
            let mut e = to128(&exp);
            while e > 0 {
                if e & 1 == 1 {
                    want = (want * b) % n128;
                }
                b = (b * b) % n128;
                e >>= 1;
            }
            assert_eq!(to128(&pow_mod(&base, &exp, &n)), want, "n={} base={}", n128, to128(&base));
        }
    }

    #[test]
    fn add_mod_reduces_full_sum() {
        let m = dec("1099511627783");
        let a = dec("1099511627782");
        let r = add_mod(&a, &dec("1"), &m);
        assert!(r.cmp(&m) == std::cmp::Ordering::Less, "add_mod result {} not < m", r.to_string());
    }

    #[test]
    fn is_prime_big_known() {
        for s in ["18446744073709551557", "1000000000000000000000007"] {
            assert!(is_prime_big(&dec(s)), "{} should be prime", s);
        }
        for s in ["18446744073709551617", "1180591620717411303427", "1208925819614629174706181"] {
            assert!(!is_prime_big(&dec(s)), "{} should be composite", s);
        }
    }

    #[test]
    fn rho_finds_proper_factor() {
        let mut rng = Rng { state: 20261107 };
        for s in ["18446744073709551617", "1208925819614629174706181"] {
            let n = dec(s);
            let d = pollard_big(&n, &mut rng);
            assert!(d.cmp(&BigU::from_u64(1)) == std::cmp::Ordering::Greater, "d=1 for {}", s);
            assert!(d.cmp(&n) == std::cmp::Ordering::Less, "d == n for {}", s);
            assert_eq!(to128(&n) % to128(&d), 0, "d={} not a divisor of {}", d.to_string(), s);
        }
    }

    #[test]
    fn factor_big_complete_factorization() {
        let mut rng = Rng { state: 20261109 };
        let mut out: Vec<BigU> = Vec::new();
        factor_big(&dec("18446744073709551617"), &mut out, &mut rng);
        let mut got: Vec<String> = out.iter().map(|x| x.to_string()).collect();
        got.sort();
        assert_eq!(got, vec!["274177".to_string(), "67280421310721".to_string()]);
    }
}
