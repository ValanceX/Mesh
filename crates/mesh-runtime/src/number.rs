//! Numbers (spec §9.7.3) and their text (§9.7.7.1).
//!
//! Arithmetic is Rust's `f64`, which is IEEE 754 binary64 under
//! roundTiesToEven, and `%` is Rust's `%`, the truncated remainder,
//! computed exactly. What needs code of its own is turning a number
//! into text.
//!
//! **Number to text** is MESH's own exact digit generator (Pass 0's
//! reviewed decision): it never uses `core::fmt`, `ryu` or any other
//! formatter, because none documents the digits MPRX requires. It is
//! Burger and Dybvig's free-format algorithm ("Printing Floating-Point
//! Numbers Quickly and Accurately", PLDI 1996) over exact integers:
//!
//! 1. The value `v`, and the interval of reals that read back as it, are
//!    `r / s`, `(r + m⁺) / s` and `(r − m⁻) / s` for integers `r`, `s`,
//!    `m⁺`, `m⁻`. Half an ulp either side, except that at a power of
//!    two the gap below is half the gap above. The ends are included
//!    when the significand is even, since a decimal exactly halfway
//!    rounds to the even neighbour.
//! 2. `s` is scaled by a power of ten, so that the interval's upper end
//!    is below 1 but at least 0.1: `v = 0.d₁d₂… × 10ⁿ`.
//! 3. Digits are generated one at a time. After each, the value with
//!    that digit (`… dₖ`) is in the interval when the remainder is at
//!    most `m⁻`, and the value with the next digit up (`… dₖ+1`) is in
//!    it when the remainder plus `m⁺` reaches `s`. Generation stops at
//!    the first digit where either is. That is the smallest `k` for which
//!    any `k`-digit candidate reads back, because the candidates are
//!    consecutive, and one of those two is nearer `v` than any other.
//! 4. The last digit is `dₖ` or `dₖ+1`, whichever is in the interval,
//!    and, when both are, the nearer to `v`: `2r` against `s`, exactly.
//!    If they are equally near, the one whose digit is even.
//!
//! That is §9.7.7.1's step 3: the shortest digit string that reads back,
//! the nearest one if there are several, the even one if two are equally
//! near. The layout is ECMA-262's. The normative table
//! (`docs/tables/number-to-text.tsv`) is what proves it; this is why it
//! should pass it.

use std::cmp::Ordering;

/// The text of a finite number, by §9.7.7.1. `-0` is `0`.
///
/// Only finite numbers have text: D8's output check stops NaN and the
/// infinities first. Given one anyway, this returns an empty string
/// rather than something that looks like a number.
pub fn number_to_text(value: f64) -> String {
    if !value.is_finite() {
        return String::new();
    }
    if value == 0.0 {
        return "0".to_string();
    }
    let (digits, point) = shortest_digits(value.abs());
    let text = layout(&digits, point);
    if value < 0.0 {
        format!("-{text}")
    } else {
        text
    }
}

/// ECMA-262's Number::toString layout, for the `k` digits of `s` and `n`
/// with the value `s × 10^(n−k)`.
fn layout(digits: &[u8], n: i32) -> String {
    let text: String = digits.iter().map(|d| char::from(b'0' + d)).collect();
    let k = i32::try_from(digits.len()).expect("at most 17 digits");
    if k <= n && n <= 21 {
        let zeros = usize::try_from(n - k).expect("n ≥ k");
        text + &"0".repeat(zeros)
    } else if 0 < n && n <= 21 {
        let point = usize::try_from(n).expect("n > 0");
        format!("{}.{}", &text[..point], &text[point..])
    } else if -6 < n && n <= 0 {
        let zeros = usize::try_from(-n).expect("n ≤ 0");
        format!("0.{}{text}", "0".repeat(zeros))
    } else {
        let exponent = n - 1;
        let sign = if exponent >= 0 { '+' } else { '-' };
        let mantissa = if k == 1 {
            text
        } else {
            format!("{}.{}", &text[..1], &text[1..])
        };
        format!("{mantissa}e{sign}{}", exponent.unsigned_abs())
    }
}

/// The shortest, nearest, ties-to-even decimal digits of a positive
/// finite `value`, and `n`: the value is `0.d₁d₂…dₖ × 10ⁿ`.
fn shortest_digits(value: f64) -> (Vec<u8>, i32) {
    const HIDDEN: u64 = 1 << 52;
    let bits = value.to_bits();
    let field = i32::try_from((bits >> 52) & 0x7ff).expect("11 bits");
    let fraction = bits & (HIDDEN - 1);
    let (m, q) = if field == 0 {
        (fraction, -1074)
    } else {
        (fraction | HIDDEN, field - 1075)
    };
    // At a power of two above the smallest normal binade, the gap below is
    // half the gap above.
    let boundary = m == HIDDEN && field > 1;
    // Ends included when the significand is even (roundTiesToEven).
    let inclusive = m % 2 == 0;

    // v = r/s; the interval is ((r − m⁻)/s, (r + m⁺)/s).
    let (mut r, mut s, mut plus, mut minus);
    if q >= 0 {
        let shift = u32::try_from(q).expect("q ≥ 0");
        let ulp = Big::from(1).shl(shift);
        if boundary {
            r = Big::from(m).shl(shift + 2);
            s = Big::from(4);
            plus = ulp.clone().shl(1);
            minus = ulp;
        } else {
            r = Big::from(m).shl(shift + 1);
            s = Big::from(2);
            plus = ulp.clone();
            minus = ulp;
        }
    } else {
        let shift = u32::try_from(-q).expect("q < 0");
        if boundary {
            r = Big::from(m).shl(2);
            s = Big::from(1).shl(shift + 2);
            plus = Big::from(2);
            minus = Big::from(1);
        } else {
            r = Big::from(m).shl(1);
            s = Big::from(1).shl(shift + 1);
            plus = Big::from(1);
            minus = Big::from(1);
        }
    }

    // `high` reaches `s` when the interval's upper end reaches 1.
    let reaches = |high: &Big, s: &Big| match high.cmp(s) {
        Ordering::Greater => true,
        Ordering::Equal => inclusive,
        Ordering::Less => false,
    };
    // Scale so that the upper end is below 1 and at least 0.1.
    let mut n = 0;
    while reaches(&r.add(&plus), &s) {
        s.mul_small(10);
        n += 1;
    }
    loop {
        let mut high = r.add(&plus);
        high.mul_small(10);
        if reaches(&high, &s) {
            break;
        }
        r.mul_small(10);
        plus.mul_small(10);
        minus.mul_small(10);
        n -= 1;
    }

    let mut digits = Vec::new();
    loop {
        r.mul_small(10);
        plus.mul_small(10);
        minus.mul_small(10);
        let mut digit = 0u8;
        while r.cmp(&s) != Ordering::Less {
            r = r.sub(&s);
            digit += 1;
        }
        let low_ok = match r.cmp(&minus) {
            Ordering::Less => true,
            Ordering::Equal => inclusive,
            Ordering::Greater => false,
        };
        let high_ok = reaches(&r.add(&plus), &s);
        match (low_ok, high_ok) {
            (false, false) => digits.push(digit),
            (true, false) => {
                digits.push(digit);
                break;
            }
            (false, true) => {
                digits.push(digit + 1);
                break;
            }
            (true, true) => {
                // The nearer of the two: compare the remainder with half
                // a unit, 2r with s. Equally near: the even digit.
                let twice = r.add(&r);
                let up = match twice.cmp(&s) {
                    Ordering::Less => false,
                    Ordering::Greater => true,
                    Ordering::Equal => digit % 2 == 1,
                };
                digits.push(if up { digit + 1 } else { digit });
                break;
            }
        }
    }
    // A last digit of 10 carries. (The scaling above means it can't carry
    // past the first digit, but a carry is handled rather than assumed
    // impossible.)
    let mut index = digits.len() - 1;
    while digits[index] == 10 {
        digits[index] = 0;
        if index == 0 {
            digits.insert(0, 1);
            n += 1;
            break;
        }
        index -= 1;
        digits[index] += 1;
    }
    while digits.len() > 1 && digits.last() == Some(&0) {
        digits.pop();
    }
    (digits, n)
}

/// An unsigned integer of any size: little-endian 32-bit limbs, with
/// exactly the operations the digit generator needs.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Big(Vec<u32>);

impl From<u64> for Big {
    fn from(value: u64) -> Self {
        #[allow(clippy::cast_possible_truncation)]
        let mut limbs = vec![value as u32, (value >> 32) as u32];
        trim(&mut limbs);
        Big(limbs)
    }
}

fn trim(limbs: &mut Vec<u32>) {
    while limbs.last() == Some(&0) {
        limbs.pop();
    }
}

impl Big {
    fn shl(mut self, bits: u32) -> Big {
        let words = usize::try_from(bits / 32).expect("a small shift");
        let bits = bits % 32;
        if bits > 0 {
            let mut carry = 0;
            for limb in &mut self.0 {
                let next = *limb >> (32 - bits);
                *limb = (*limb << bits) | carry;
                carry = next;
            }
            if carry > 0 {
                self.0.push(carry);
            }
        }
        let mut limbs = vec![0; words];
        limbs.extend(self.0);
        trim(&mut limbs);
        Big(limbs)
    }

    fn mul_small(&mut self, factor: u32) {
        let mut carry = 0u64;
        for limb in &mut self.0 {
            let product = u64::from(*limb) * u64::from(factor) + carry;
            #[allow(clippy::cast_possible_truncation)]
            {
                *limb = product as u32;
            }
            carry = product >> 32;
        }
        if carry > 0 {
            #[allow(clippy::cast_possible_truncation)]
            self.0.push(carry as u32);
        }
    }

    fn add(&self, other: &Big) -> Big {
        let mut limbs = Vec::with_capacity(self.0.len().max(other.0.len()) + 1);
        let mut carry = 0u64;
        for index in 0..self.0.len().max(other.0.len()) {
            let sum = u64::from(self.0.get(index).copied().unwrap_or(0))
                + u64::from(other.0.get(index).copied().unwrap_or(0))
                + carry;
            #[allow(clippy::cast_possible_truncation)]
            limbs.push(sum as u32);
            carry = sum >> 32;
        }
        if carry > 0 {
            #[allow(clippy::cast_possible_truncation)]
            limbs.push(carry as u32);
        }
        Big(limbs)
    }

    /// `self − other`, which must not be negative.
    fn sub(&self, other: &Big) -> Big {
        let mut limbs = Vec::with_capacity(self.0.len());
        let mut borrow = 0i64;
        for index in 0..self.0.len() {
            let mut difference = i64::from(self.0[index])
                - i64::from(other.0.get(index).copied().unwrap_or(0))
                - borrow;
            borrow = i64::from(difference < 0);
            if difference < 0 {
                difference += 1 << 32;
            }
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            limbs.push(difference as u32);
        }
        debug_assert_eq!(borrow, 0, "a subtraction that would be negative");
        trim(&mut limbs);
        Big(limbs)
    }
}

impl PartialOrd for Big {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Big {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0
            .len()
            .cmp(&other.0.len())
            .then_with(|| self.0.iter().rev().cmp(other.0.iter().rev()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn big_integers_do_arithmetic() {
        let a = Big::from(u64::MAX);
        let b = a.add(&Big::from(1));
        assert_eq!(b, Big::from(1).shl(64));
        assert_eq!(b.sub(&Big::from(1)), a);
        let mut c = Big::from(1);
        for _ in 0..30 {
            c.mul_small(10);
        }
        assert!(c > Big::from(1).shl(99) && c < Big::from(1).shl(100));
        assert_eq!(Big::from(0), Big(Vec::new()));
    }

    #[test]
    fn layout_follows_ecma_262() {
        assert_eq!(layout(&[1, 5], 1), "1.5");
        assert_eq!(layout(&[1], 3), "100");
        assert_eq!(layout(&[1], 0), "0.1");
        assert_eq!(layout(&[1], -5), "0.000001");
        assert_eq!(layout(&[1], -6), "1e-7");
        assert_eq!(layout(&[1], 22), "1e+21");
        assert_eq!(layout(&[1, 2, 5], 31), "1.25e+30");
    }
}
