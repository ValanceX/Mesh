//! A non-normative check of number to text over generated values: the
//! text reads back as the value, and no shorter digit string would. It
//! catches gross breakage between the normative table's rows; only the
//! table proves "nearest" and "even" (outline D8).

use mesh_runtime::number_to_text;

/// xorshift64, as the other fuzz tests use: the same everywhere.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// The significant digits of a MESH number text, without sign, point,
/// exponent or leading and trailing zeros.
fn significant(text: &str) -> String {
    let mantissa = text.trim_start_matches('-').split('e').next().unwrap();
    mantissa
        .replace('.', "")
        .trim_start_matches('0')
        .trim_end_matches('0')
        .to_string()
}

#[test]
fn texts_read_back_and_are_shortest() {
    let mut rng = Rng(0x4d45_5348_7465_7874);
    // `MESH_TEXT_VALUES` sets how many; 20,000 by default.
    let count: usize = std::env::var("MESH_TEXT_VALUES")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(20_000);
    let mut checked = 0;
    while checked < count {
        let value = f64::from_bits(rng.next());
        if !value.is_finite() {
            continue;
        }
        let text = number_to_text(value);
        let read: f64 = text.parse().unwrap_or_else(|_| panic!("{text} parses"));
        assert_eq!(read, value, "{text} reads back as {value:e}");
        if value != 0.0 {
            let digits = significant(&text);
            if digits.len() > 1 {
                // The shorter string nearest the value is it correctly
                // rounded to one digit fewer, and the values that read
                // back form an interval around the value: if that one
                // doesn't read back, no shorter string does. (std's
                // fixed-precision formatting is only a checking tool
                // here; it never produces MESH text.)
                let shorter = format!("{:.*e}", digits.len() - 2, value.abs());
                let read: f64 = shorter.parse().unwrap();
                assert_ne!(
                    read,
                    value.abs(),
                    "{shorter} also reads back as {value:e}, but MESH gave {text}"
                );
            }
        }
        checked += 1;
    }
}
