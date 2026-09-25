//! Number to text: the normative verification (outline D8; spec
//! §9.7.7.1). The runtime must reproduce every row of the normative table
//! exactly. The table is read from `docs/tables/number-to-text.tsv`
//! itself, never a copy, and no row is skipped.

use mesh_runtime::number_to_text;
use std::fs;
use std::path::Path;

const NON_FINITE: &str = "!runtime-non-finite-output";

struct Row {
    bits: u64,
    expected: String,
    category: String,
    why: String,
}

fn table() -> Vec<Row> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/tables/number-to-text.tsv");
    let text = fs::read_to_string(&path).expect("the normative table reads");
    let mut lines = text.lines();
    assert_eq!(
        lines.next(),
        Some("bits\texpected\tcategory\twhy"),
        "the table's header"
    );
    lines
        .map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            assert_eq!(fields.len(), 4, "a row has four fields: {line:?}");
            Row {
                bits: u64::from_str_radix(fields[0], 16).expect("bits are hexadecimal"),
                expected: fields[1].to_string(),
                category: fields[2].to_string(),
                why: fields[3].to_string(),
            }
        })
        .collect()
}

/// Every finite row gives exactly its text. (The non-finite rows are
/// checked at the render level, in `evaluation.rs`: they never reach
/// text conversion.)
#[test]
fn the_normative_table_holds() {
    let mut failures = Vec::new();
    let mut checked = 0;
    for row in table() {
        let value = f64::from_bits(row.bits);
        if row.expected == NON_FINITE {
            assert!(
                !value.is_finite(),
                "{:016x} is finite but expects {NON_FINITE}",
                row.bits
            );
            continue;
        }
        let text = number_to_text(value);
        if text != row.expected {
            failures.push(format!(
                "{:016x} [{}] {}: expected {}, got {text}",
                row.bits, row.category, row.why, row.expected
            ));
        }
        checked += 1;
    }
    assert!(
        failures.is_empty(),
        "{} rows fail:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(checked >= 80, "only {checked} finite rows");
}

/// Every category Pass 0 listed has rows, so the table can't lose one
/// silently.
#[test]
fn every_category_is_present() {
    let rows = table();
    for category in [
        "ordinary",
        "zero",
        "integer-boundary",
        "layout-threshold",
        "magnitude",
        "subnormal",
        "nearest-choice",
        "tie",
        "non-finite",
    ] {
        assert!(
            rows.iter().any(|row| row.category == category),
            "the table has no {category} row"
        );
    }
}

/// The spec's own examples (§9.7.7.1), one assertion each.
#[test]
fn the_specs_examples_hold() {
    for (value, text) in [
        (1.5, "1.5"),
        (100.0, "100"),
        (0.1, "0.1"),
        (-2.5, "-2.5"),
        (0.000001, "0.000001"),
        (1e-7, "1e-7"),
        (123456789012345680000.0, "123456789012345680000"),
        (1e21, "1e+21"),
        (0.1 + 0.2, "0.30000000000000004"),
        (f64::from_bits(1), "5e-324"),
        (-0.0, "0"),
        (0.0, "0"),
        (2f64.powi(-25), "2.9802322387695312e-8"),
    ] {
        assert_eq!(number_to_text(value), text, "{value:e}");
    }
}
