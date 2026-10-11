#!/usr/bin/env python3
"""stat --printf escape probe: candidate parser vs reference (uutils coreutils 0.10.0).

Candidate algorithm (validated by this probe; used by src/applets/stat.rs):
  recognized single-char: \\a \\b \\f \\n \\r \\t \\\\ \\\" \\'   -> byte
  \\x:
    - next char is hex digit: take 2 hex digits when both of the next two are hex,
      else take the single hex digit -> byte
    - else: warning; message is "incomplete hex escape '\\x'" when the format ends
      right after \\x, otherwise "unrecognized escape '\\x'"; emit literal 'x'
  \\0..\\7: consume a run of octal digits capped at 3 -> byte (value & 0o377)
  \\8, \\9 or any other char: warning "unrecognized escape '\\<c>'", emit the char literally
Warnings are printed to stderr as `stat: warning: <message>` at the point of emission.
"""
import os, random, subprocess, sys

REF = "/usr/lib/cargo/bin/coreutils/stat"
P = "/tmp/statp/rf"
HEX = "0123456789abcdefABCDEF"
OCT = "01234567"
SINGLE = {"a": 7, "b": 8, "f": 12, "n": 10, "r": 13, "t": 9, "\\": 92, '"': 34, "'": 39}


def candidate(s: str):
    out = bytearray()
    warns = []
    i = 0
    n = len(s)
    while i < n:
        c = s[i]
        if c != "\\":
            out += c.encode()
            i += 1
            continue
        if i + 1 >= n:
            out += b"\\"
            warns.append("backslash at end of format")
            i += 1
            continue
        c2 = s[i + 1]
        if c2 in SINGLE:
            out.append(SINGLE[c2])
            i += 2
            continue
        if c2 == "x":
            if i + 2 < n and s[i + 2] in HEX and (i + 3 >= n and False or True):
                if i + 3 < n and s[i + 3] in HEX:
                    out.append(int(s[i + 2] + s[i + 3], 16))
                    i += 4
                else:
                    out.append(int(s[i + 2], 16))
                    i += 3
                continue
            if i + 2 >= n:
                warns.append("incomplete hex escape '\\x'")
            else:
                warns.append("unrecognized escape '\\x'")
            out.append(ord("x"))
            i += 2
            continue
        if c2 in OCT:
            j = i + 1
            while j < n and j - (i + 1) < 3 and s[j] in OCT:
                j += 1
            out.append(int(s[i + 1 : j], 8) & 0o377)
            i = j
            continue
        warns.append("unrecognized escape '\\%s'" % c2)
        out.append(ord(c2))
        i += 2
    return bytes(out), warns


def _preexec():
    import resource
    resource.setrlimit(resource.RLIMIT_AS, (1 << 30, 1 << 30))
    os.umask(0o022)


def ref(fmt):
    p = subprocess.run([REF, "--printf", fmt, P], capture_output=True, timeout=60,
                       stdin=subprocess.DEVNULL, preexec_fn=_preexec)
    return p.stdout, p.stderr


def main():
    os.makedirs("/tmp/statp", exist_ok=True)
    os.umask(0o022)
    open(P, "w").write("hi\n")
    cases = []
    for i in range(32, 127):
        c = chr(i)
        for tail in ["", "b", "5", "z", "{X}", "\\n", "1", "8", "x", "01", "123", "400", "100", "ff", "g8"]:
            cases.append("\\" + c + tail)
    for h1 in HEX:
        for h2 in HEX:
            cases.append("\\x%s%s" % (h1, h2))
            cases.append("\\x%s%sz" % (h1, h2))
    random.seed(20261104)
    alpha = "abfnrtvexuUNZ0123456789{}\\%zxg"
    for _ in range(700):
        cases.append("".join(random.choice(["\\", "a", "b"]) for _ in range(random.randint(1, 4)))
                     + "".join(random.choice(alpha) for _ in range(random.randint(0, 4))))
    bad = 0
    shown = 0
    seen = set()
    for fmt in cases:
        if fmt in seen or "%" in fmt:
            continue
        seen.add(fmt)
        o, e = ref(fmt)
        co, cw = candidate(fmt)
        ce = "".join("stat: warning: %s\n" % w for w in cw).encode()
        if (o, e) != (co, ce):
            bad += 1
            if shown < 20:
                shown += 1
                print("MISMATCH %r ref=(%r,%r) cand=(%r,%r)" % (fmt, o, e, co, ce))
    print("TOTAL %d escape cases, %d mismatches" % (len(seen), bad))
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
