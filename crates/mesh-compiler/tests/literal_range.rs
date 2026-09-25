//! `number-literal-out-of-range` (§9.7.3): a number literal whose
//! nearest binary64 value would be infinite is an error, reported by
//! lowering, so with or without a component model.

use mesh_compiler::{compile, CompileResult};
use mesh_syntax::{DiagnosticCode, Severity, Span};

/// The exact decimal value of the largest finite binary64 value.
const MAX: &str = "179769313486231570814527423731704356798070567525844996598917476803157260780028538760589558632766878171540458953514382464234321326889464182768467546703537516986049910576551282076245490090389328944075868508455133942304583236903222948165808559332123348274797826204144723168738177180919299881250404026184124858368";

fn codes(result: &CompileResult) -> Vec<&'static str> {
    result.diagnostics.iter().map(|d| d.code.as_str()).collect()
}

fn span_of(source: &str, needle: &str) -> Span {
    let start_byte = source.find(needle).expect("the needle is in the source");
    Span {
        start_byte,
        end_byte: start_byte + needle.len(),
    }
}

#[test]
fn a_literal_too_large_to_be_finite_is_an_error_at_the_literal() {
    let nines = "9".repeat(309);
    let source = format!("<page title={{{nines}}} />");
    let result = compile(&source);
    assert_eq!(codes(&result), ["number-literal-out-of-range"]);
    let diagnostic = &result.diagnostics[0];
    assert_eq!(diagnostic.severity, Severity::Error);
    assert_eq!(diagnostic.code, DiagnosticCode::NUMBER_LITERAL_OUT_OF_RANGE);
    assert_eq!(diagnostic.span, span_of(&source, &nines));
    assert!(result.ir.is_some(), "lowering still produces IR");
}

#[test]
fn the_largest_finite_value_and_what_rounds_to_it_are_not() {
    assert_eq!(MAX.len(), 309);
    // MAX + 1 is below the midpoint between MAX and 2^1024, so it rounds
    // down to MAX.
    let above: String = {
        let mut digits: Vec<u8> = MAX.bytes().collect();
        *digits.last_mut().unwrap() += 1; // MAX ends in 8
        String::from_utf8(digits).unwrap()
    };
    for literal in [MAX.to_string(), above, format!("{MAX}.999")] {
        let source = format!("<page title={{{literal}}} />");
        assert!(compile(&source).diagnostics.is_empty(), "{literal}");
    }
}

#[test]
fn every_out_of_range_literal_is_reported_wherever_it_is() {
    let big = "1".to_string() + &"0".repeat(309);
    let source =
        format!("<page a={{-({big} + 1)}} on.x={{go([{big}])}}><text>{{{big}}}</text></page>");
    let result = compile(&source);
    assert_eq!(
        codes(&result),
        ["number-literal-out-of-range"; 3],
        "{:#?}",
        result.diagnostics
    );
}
