#!/usr/bin/env python3
"""The reference generator for MESH's normative number-to-text table.

MPRX converts a finite number to text by the rule in the spec's section
9.7 (docs/MPRX-SPEC.md): the shortest decimal digit string that reads
back as the same binary64 value; if several, the one nearest the exact
value; if two are equally near, the one whose last digit is even; laid
out by ECMA-262's Number::toString.

This script implements that rule literally, with exact rational
arithmetic, and writes `number-to-text.tsv` beside it. It never formats
or parses a float: every value is given as a bit pattern, or as a
decimal string that `bits_of` rounds to binary64 exactly, and nothing
calls `float`, `repr`, `str` of a float, `format` or `float.hex`. So the
table records what the rule says, not what any implementation prints.

    python3 docs/tables/number_to_text.py --write         # regenerate the table
    python3 docs/tables/number_to_text.py --check         # the table is what the rule gives
    python3 docs/tables/number_to_text.py --search-ties   # look for equally near candidates
    python3 docs/tables/number_to_text.py --explain BITS  # every candidate for one value

Standard library only. Deterministic: random values come from fixed seeds.
"""

import argparse
import random
import sys
from fractions import Fraction
from pathlib import Path

TABLE = Path(__file__).with_name("number-to-text.tsv")
HEADER = "bits\texpected\tcategory\twhy"
NON_FINITE = "!runtime-non-finite-output"

# --- binary64 -------------------------------------------------------------

EXPONENT_MASK = 0x7FF
FRACTION_BITS = 52
HIDDEN = 1 << FRACTION_BITS
MIN_EXPONENT = -1074  # the exponent of the least significant bit of a subnormal


def decode(bits):
    """(sign, significand m, exponent q) with value (-1)^sign * m * 2^q,
    or None for NaN and the infinities."""
    sign = bits >> 63
    exponent = (bits >> FRACTION_BITS) & EXPONENT_MASK
    fraction = bits & (HIDDEN - 1)
    if exponent == EXPONENT_MASK:
        return None
    if exponent == 0:
        return sign, fraction, MIN_EXPONENT
    return sign, fraction | HIDDEN, exponent - 1075


def value_of(bits):
    sign, m, q = decode(bits)
    v = Fraction(m) * Fraction(2) ** q
    return -v if sign else v


def encode(sign, m, q):
    """Bits of (-1)^sign * m * 2^q, for m and q as decode returns them."""
    if m == 0:
        return sign << 63
    if m < HIDDEN:
        assert q == MIN_EXPONENT
        return (sign << 63) | m
    return (sign << 63) | ((q + 1075) << FRACTION_BITS) | (m - HIDDEN)


def round_to_binary64(x):
    """Bits of the binary64 value nearest the positive rational x, ties to
    even (IEEE 754 roundTiesToEven); None if that is infinite."""
    assert x > 0
    # Find q so that m = x / 2^q lies in [2^52, 2^53), or q = -1074.
    q = x.numerator.bit_length() - x.denominator.bit_length() - FRACTION_BITS
    while Fraction(2) ** q * HIDDEN > x:
        q -= 1
    while Fraction(2) ** (q + 1) * HIDDEN <= x:
        q += 1
    q = max(q, MIN_EXPONENT)
    scaled = x / Fraction(2) ** q
    m = scaled.numerator // scaled.denominator
    remainder = scaled - m
    if remainder > Fraction(1, 2) or (remainder == Fraction(1, 2) and m % 2 == 1):
        m += 1
    if m == 2 * HIDDEN:
        m, q = HIDDEN, q + 1
    if q + 1075 >= EXPONENT_MASK:
        return None
    return encode(0, m, q)


def bits_of(decimal):
    """Bits of the binary64 value nearest a decimal string, like "-2.5e-3"."""
    text = decimal.strip().lower()
    sign = 0
    if text.startswith("-"):
        sign, text = 1, text[1:]
    mantissa, _, exponent = text.partition("e")
    whole, _, fraction = mantissa.partition(".")
    digits = int(whole + fraction or "0")
    x = Fraction(digits) * Fraction(10) ** (int(exponent or "0") - len(fraction))
    if x == 0:
        return sign << 63
    bits = round_to_binary64(x)
    assert bits is not None, decimal
    return bits | (sign << 63)


def neighbours(bits):
    """The positive values just below and just above a positive finite one."""
    return bits - 1, bits + 1


# --- the rule ---------------------------------------------------------------


def interval(bits):
    """The rounding interval of a positive finite value: (low, high,
    inclusive). A rational reads back as the value exactly when it lies in
    it; the ends are included when the significand is even (a decimal
    exactly halfway rounds to the even neighbour)."""
    _, m, q = decode(bits)
    v = Fraction(m) * Fraction(2) ** q
    ulp = Fraction(2) ** q
    upper = v + ulp / 2
    # At a power of two (above the smallest normal binade) the gap below
    # is half the gap above.
    if m == HIDDEN and q > MIN_EXPONENT:
        lower = v - ulp / 4
    else:
        lower = v - ulp / 2
    return lower, upper, m % 2 == 0


def floor_log10(x):
    e = len(str(x.numerator)) - len(str(x.denominator))
    while Fraction(10) ** e > x:
        e -= 1
    while Fraction(10) ** (e + 1) <= x:
        e += 1
    return e


def candidates(bits):
    """(k, [(s, e) ...], value): the shortest digit count k, every k-digit
    s with s * 10^e reading back as the value, and the value itself."""
    v = value_of(bits)
    low, high, inclusive = interval(bits)

    def inside(c):
        return (low <= c <= high) if inclusive else (low < c < high)

    top = floor_log10(v)
    for k in range(1, 18):
        found = []
        for e in range(top - k - 1, top - k + 3):
            unit = Fraction(10) ** e
            first = -((-low) // unit)  # ceil
            last = high // unit
            for s in range(max(first, 10 ** (k - 1)), min(last, 10 ** k - 1) + 1):
                if inside(s * unit):
                    found.append((s, e))
        if found:
            return k, sorted(set(found), key=lambda c: c[0] * Fraction(10) ** c[1]), v
    raise AssertionError("no candidate within 17 digits")


def choose(bits):
    """The chosen (s, e), and whether the choice was an exact tie."""
    _, found, v = candidates(bits)
    best = min(abs(s * Fraction(10) ** e - v) for s, e in found)
    nearest = [(s, e) for s, e in found if abs(s * Fraction(10) ** e - v) == best]
    if len(nearest) == 1:
        return nearest[0], False
    even = [(s, e) for s, e in nearest if s % 2 == 0]
    assert len(nearest) == 2 and len(even) == 1, nearest
    return even[0], True


def layout(s, e):
    """ECMA-262 Number::toString(x, 10), steps after the digits are chosen:
    k digits of s, and n with the value s * 10^(n - k)."""
    digits = str(s)
    k = len(digits)
    n = e + k
    if k <= n <= 21:
        return digits + "0" * (n - k)
    if 0 < n <= 21:
        return digits[:n] + "." + digits[n:]
    if -6 < n <= 0:
        return "0." + "0" * (-n) + digits
    exponent = n - 1
    sign = "+" if exponent >= 0 else "-"
    mantissa = digits if k == 1 else digits[0] + "." + digits[1:]
    return mantissa + "e" + sign + str(abs(exponent))


def to_text(bits):
    """MESH's text for a binary64 value, or NON_FINITE for NaN and the
    infinities, which D8's output check stops before text conversion."""
    if decode(bits) is None:
        return NON_FINITE
    if bits & ~(1 << 63) == 0:
        return "0"  # +0 and -0 alike
    negative = bits >> 63
    (s, e), _ = choose(bits & ~(1 << 63))
    text = layout(s, e)
    return "-" + text if negative else text


# --- the tie search ---------------------------------------------------------


def search_ties(samples_per_pair=3, seed=5, exponents=range(MIN_EXPONENT, 0)):
    """Look for values whose two nearest shortest candidates are exactly
    equally near.

    Such a value v is the midpoint (s + 1/2) * 10^j of two k-digit
    decimals. Writing v = m * 2^q: for j >= 0, v's 2-adic valuation is
    j - 1, so q <= j - 1, but both candidates reading back needs the
    rounding interval, at most 2^q wide, to be at least 10^j wide, and
    10^j > 2^(j-1). So j < 0, and then v = r * 2^t * 2^q with r odd and
    t = -q - 1 - |j| >= 0, and 10^j <= 2^q. The search walks every binary
    exponent q, every j allowed by those bounds, and sample odd r, and
    runs the rule on each value it builds, recording those whose choice
    was a tie."""
    rng = random.Random(seed)
    found, tried = [], 0
    for q in exponents:
        for a in range(1, -q):  # a = |j|
            if Fraction(10) ** -a > Fraction(2) ** q:
                continue
            t = -q - 1 - a
            if t > FRACTION_BITS:
                continue
            # m = r * 2^t with r odd and 5^a | (2s + 1): the midpoint's
            # numerator; r ranges over odd multiples of nothing further,
            # since v's denominator is already a power of two.
            lo = 1 if q == MIN_EXPONENT else HIDDEN
            r_lo = -(-lo // (1 << t))
            r_hi = (2 * HIDDEN - 1) >> t
            if r_lo > r_hi:
                continue
            picks = {r_lo, r_hi} | {rng.randint(r_lo, r_hi) for _ in range(samples_per_pair)}
            for r in picks:
                if r % 2 == 0:
                    r += 1 if r + 1 <= r_hi else -1
                if not r_lo <= r <= r_hi:
                    continue
                m = r << t
                if q != MIN_EXPONENT and not HIDDEN <= m < 2 * HIDDEN:
                    continue
                if q == MIN_EXPONENT and m >= HIDDEN:
                    continue
                bits = encode(0, m, q)
                tried += 1
                if choose(bits)[1] and bits not in found:
                    found.append(bits)
    return found, tried


# --- the table ----------------------------------------------------------------


def rows():
    """(bits, category, why) for every row, in table order."""
    out = []

    def add(bits, category, why):
        if all(bits != seen for seen, _, _ in out):
            out.append((bits, category, why))

    def both_signs(decimal, category, why):
        add(bits_of(decimal), category, why)
        add(bits_of("-" + decimal), category, why + ", negated")

    for decimal, why in [
        ("1.5", "a short fraction"),
        ("100", "an integer with trailing zeros"),
        ("0.1", "not exactly representable; the shortest form is one digit"),
        ("2.5", "the spec's example -2.5, and its negation"),
        ("123.456", "an ordinary three-place decimal"),
        ("1", "one"),
        ("42", "an ordinary integer"),
    ]:
        both_signs(decimal, "ordinary", why)
    third = round_to_binary64(Fraction(1, 3))
    add(third, "ordinary", "1/3, seventeen digits")
    point3 = round_to_binary64(value_of(bits_of("0.1")) + value_of(bits_of("0.2")))
    add(point3, "ordinary", "0.1 + 0.2 rounded, which is not 0.3")

    add(0, "zero", "+0")
    add(1 << 63, "zero", "-0 is 0 in text (D8)")

    for n in (1, 52, 53, 54, 63, 64, 1023):
        p = bits_of(str(2 ** n))
        add(p, "integer-boundary", f"2^{n}, where the gap below is half the gap above")
        low, high = neighbours(p)
        add(low, "integer-boundary", f"the largest value below 2^{n}")
        add(high, "integer-boundary", f"the smallest value above 2^{n}")
    add(bits_of(str(2 ** 53 - 1)), "integer-boundary", "2^53 - 1, the largest integer with all neighbours representable")
    add(bits_of(str(2 ** 53 + 2)), "integer-boundary", "2^53 + 2, the next representable integer after 2^53")

    below_21 = neighbours(bits_of("1e21"))[0]
    for bits, why in [
        (bits_of("123456789012345680000"), "n = 21: the last plain integer layout"),
        (bits_of("1e21"), "n = 22: the first exponent layout for large numbers"),
        (below_21, "the largest value below 1e21, still plain"),
        (bits_of("0.000001"), "n = -5: the last plain fraction layout"),
        (bits_of("1e-7"), "n = -6: the first exponent layout for small numbers"),
        (neighbours(bits_of("0.000001"))[0], "the largest value below 1e-6"),
        (neighbours(bits_of("1e-7"))[1], "the smallest value above 1e-7"),
        (bits_of("1.5e-7"), "a two-digit mantissa in the negative exponent layout"),
        (bits_of("1.25e+30"), "a three-digit mantissa in the positive exponent layout"),
        (bits_of("0.00123"), "-6 < n <= 0, with leading zeros"),
        (bits_of("12.5"), "0 < n < k, a point inside the digits"),
    ]:
        add(bits, "layout-threshold", why)

    largest = 0x7FEFFFFFFFFFFFFF
    smallest_normal = 0x0010000000000000
    for bits, why in [
        (largest, "the largest finite value"),
        (largest ^ (1 << 63), "the most negative finite value"),
        (bits_of("1e308"), "a large power of ten"),
        (bits_of("1e-300"), "a small power of ten"),
        (smallest_normal, "the smallest normal value"),
        (smallest_normal - 1, "the largest subnormal, just below the smallest normal"),
        (smallest_normal + 1, "just above the smallest normal value"),
    ]:
        add(bits, "magnitude", why)

    rng = random.Random(1)
    add(1, "subnormal", "the smallest subnormal: 3e-324 to 7e-324 all read back; 5 is nearest")
    add(smallest_normal - 1, "subnormal", "the largest subnormal")
    add(2, "subnormal", "twice the smallest subnormal")
    for _ in range(3):
        add(rng.randint(3, smallest_normal - 2), "subnormal", "a random subnormal")

    # Values where more than one shortest candidate reads back, so the
    # nearest one decides: found by scanning, and kept to show both
    # directions (nearest below the value, and above it).
    rng = random.Random(2)
    below = above = 0
    picked = []
    while len(picked) < 12 or below == 0 or above == 0:
        bits = rng.getrandbits(63)
        if decode(bits) is None or bits == 0:
            continue
        k, found, v = candidates(bits)
        if len(found) < 2:
            continue
        (s, e), _ = choose(bits)
        c = s * Fraction(10) ** e
        side = "below" if c < v else "above"
        if side == "below" and below >= 6 or side == "above" and above >= 6:
            continue
        below += side == "below"
        above += side == "above"
        picked.append((bits, f"{len(found)} {k}-digit candidates read back; the nearest is {side} the value"))
    for bits, why in picked:
        add(bits, "nearest-choice", why)

    # A bounded search keeps --check fast; --search-ties runs the whole one.
    ties = search_ties(samples_per_pair=1, exponents=range(-60, -20))[0]
    assert ties, "the bounded search found no tie"
    step = max(1, len(ties) // 8)
    for bits in ties[::step][:8]:
        (s, e), _ = choose(bits)
        add(bits, "tie", f"the value is exactly halfway between two shortest candidates; the even one ({str(s)[-1]}) is chosen")

    add(0x7FF8000000000000, "non-finite", "a quiet NaN")
    add(0xFFF8000000000001, "non-finite", "a NaN with a payload and the sign bit set")
    add(0x7FF0000000000000, "non-finite", "+infinity")
    add(0xFFF0000000000000, "non-finite", "-infinity")
    return out


def table_text():
    lines = [HEADER]
    for bits, category, why in rows():
        lines.append(f"{bits:016x}\t{to_text(bits)}\t{category}\t{why}")
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--write", action="store_true")
    group.add_argument("--check", action="store_true")
    group.add_argument("--search-ties", action="store_true")
    group.add_argument("--explain", metavar="BITS")
    args = parser.parse_args()
    if args.write:
        TABLE.write_text(table_text())
    elif args.check:
        if TABLE.read_text() != table_text():
            sys.exit(f"{TABLE} isn't what the rule gives: run --write and review the diff")
        print(f"{TABLE.name} is what the rule gives")
    elif args.search_ties:
        found, tried = search_ties()
        print(f"tried {tried} constructed midpoints; {len(found)} were ties")
        for bits in found:
            print(f"{bits:016x}\t{to_text(bits)}")
    else:
        bits = int(args.explain, 16)
        k, found, v = candidates(bits & ~(1 << 63))
        print(f"value = {v}\nshortest: {k} digits")
        for s, e in found:
            print(f"  {s}e{e}: distance {abs(s * Fraction(10) ** e - v)}")
        print("text:", to_text(bits))


if __name__ == "__main__":
    main()
