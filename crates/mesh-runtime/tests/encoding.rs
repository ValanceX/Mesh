//! The byte encoding a JavaScript host's values cross in
//! (`mesh_runtime::encoding`, Pass 3 Decision 7): every tag decodes to
//! its `HostValue`, bits are kept exactly, and bad bytes are an error,
//! never a panic.

use mesh_runtime::encoding::{
    decode_snapshot, decode_texts, decode_value, encode_texts, encode_value,
};
use mesh_runtime::{HostKey, HostRecord, HostValue};

fn string_body(units: &[u16]) -> Vec<u8> {
    let mut bytes = (units.len() as u32).to_le_bytes().to_vec();
    for unit in units {
        bytes.extend(unit.to_le_bytes());
    }
    bytes
}

fn utf16(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

#[test]
fn every_tag_decodes_to_its_value() {
    assert_eq!(decode_value(&[1]).unwrap(), HostValue::Null);
    assert_eq!(decode_value(&[2]).unwrap(), HostValue::Boolean(false));
    assert_eq!(decode_value(&[3]).unwrap(), HostValue::Boolean(true));

    let mut number = vec![4];
    number.extend(1.5f64.to_bits().to_le_bytes());
    assert_eq!(decode_value(&number).unwrap(), HostValue::Number(1.5));

    let mut string = vec![5];
    string.extend(string_body(&utf16("Ada 😀")));
    assert_eq!(
        decode_value(&string).unwrap(),
        HostValue::String("Ada 😀".into())
    );

    // A list with a null, a hole and a true.
    let mut list = vec![6];
    list.extend(3u32.to_le_bytes());
    list.extend([1, 0, 3]);
    assert_eq!(
        decode_value(&list).unwrap(),
        HostValue::List(vec![
            Some(HostValue::Null),
            None,
            Some(HostValue::Boolean(true))
        ])
    );

    let mut record = vec![7];
    record.extend(2u32.to_le_bytes());
    record.extend(string_body(&utf16("b")));
    record.push(1);
    record.extend(string_body(&utf16("a")));
    record.push(2);
    assert_eq!(
        decode_value(&record).unwrap(),
        HostValue::Record(HostRecord(vec![
            (HostKey::Text("b".into()), HostValue::Null),
            (HostKey::Text("a".into()), HostValue::Boolean(false)),
        ])),
        "fields keep the host's order"
    );

    let mut unsupported = vec![8];
    unsupported.extend(string_body(&utf16("object:Map")));
    assert_eq!(
        decode_value(&unsupported).unwrap(),
        HostValue::Unsupported("object:Map".into())
    );
}

#[test]
fn an_unpaired_surrogate_is_kept_as_utf16() {
    let mut string = vec![5];
    string.extend(string_body(&[0x61, 0xd800]));
    assert_eq!(
        decode_value(&string).unwrap(),
        HostValue::Utf16(vec![0x61, 0xd800])
    );
    let mut record = vec![7];
    record.extend(1u32.to_le_bytes());
    record.extend(string_body(&[0xdc00]));
    record.push(1);
    assert_eq!(
        decode_value(&record).unwrap(),
        HostValue::Record(HostRecord(vec![(
            HostKey::Utf16(vec![0xdc00]),
            HostValue::Null
        )]))
    );
}

#[test]
fn numbers_keep_their_bits() {
    for number in [
        f64::NAN,
        -f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -0.0,
        0.0,
        5e-324,
    ] {
        let HostValue::Number(decoded) =
            decode_value(&encode_value(&HostValue::Number(number))).unwrap()
        else {
            panic!("a number");
        };
        assert_eq!(decoded.to_bits(), number.to_bits());
    }
}

#[test]
fn encoding_and_decoding_round_trip() {
    let value = HostValue::Record(HostRecord(vec![
        (
            HostKey::Text("users".into()),
            HostValue::List(vec![
                Some(HostValue::Record(HostRecord(vec![(
                    HostKey::Text("name".into()),
                    HostValue::String("Grace".into()),
                )]))),
                None,
            ]),
        ),
        (HostKey::Utf16(vec![0xd83d]), HostValue::Utf16(vec![0xde00])),
        (
            HostKey::Text("f".into()),
            HostValue::Unsupported("function".into()),
        ),
    ]));
    assert_eq!(decode_value(&encode_value(&value)).unwrap(), value);
    let texts = ["{\"a\":1}", "", "日本"];
    assert_eq!(decode_texts(&encode_texts(&texts)).unwrap(), texts);
}

#[test]
fn a_snapshot_is_a_record() {
    assert!(decode_snapshot(&[1]).is_err());
    let mut empty = vec![7];
    empty.extend(0u32.to_le_bytes());
    assert_eq!(decode_snapshot(&empty).unwrap(), HostRecord::default());
}

#[test]
fn bad_bytes_are_errors() {
    let bad: &[&[u8]] = &[
        &[],
        &[0],             // a hole outside a list
        &[9],             // no such tag
        &[4, 0, 0],       // a truncated number
        &[1, 1],          // trailing bytes
        &[5, 1, 0, 0, 0], // a truncated string
        &[6, 255, 255, 255, 255],
        &[7, 1, 0, 0, 0, 0, 0, 0, 0, 0], // a field whose value is a hole
        &[8, 1, 0, 0, 0, 0, 0xd8],       // a kind that isn't text
    ];
    for bytes in bad {
        assert!(decode_value(bytes).is_err(), "{bytes:?}");
    }
    for bytes in [
        &[][..],
        &[1, 0, 0, 0],
        &[1, 0, 0, 0, 1, 0, 0, 0, 0xff],
        &[0, 0, 0, 0, 1],
    ] {
        assert!(decode_texts(bytes).is_err(), "{bytes:?}");
    }
}

#[test]
fn nesting_is_limited() {
    let deep = |levels: usize| {
        let mut bytes = Vec::new();
        for _ in 0..levels {
            bytes.push(6);
            bytes.extend(1u32.to_le_bytes());
        }
        bytes.push(1);
        bytes
    };
    assert!(decode_value(&deep(128)).is_ok());
    assert!(decode_value(&deep(129)).is_err());
    assert!(
        decode_value(&deep(100_000)).is_err(),
        "and never overflows the stack"
    );
}

#[test]
fn mutated_encodings_never_panic() {
    let seed = encode_value(&HostValue::Record(HostRecord(vec![
        (
            HostKey::Text("a".into()),
            HostValue::List(vec![Some(HostValue::Number(1.0)), None]),
        ),
        (HostKey::Text("b".into()), HostValue::String("x😀".into())),
    ])));
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..20_000 {
        let mut bytes = seed.clone();
        for _ in 0..1 + next() % 3 {
            let at = (next() as usize) % bytes.len();
            match next() % 3 {
                0 => bytes[at] = next() as u8,
                1 => {
                    bytes.remove(at);
                }
                _ => bytes.truncate(at),
            }
            if bytes.is_empty() {
                break;
            }
        }
        let _ = decode_value(&bytes);
        let _ = decode_texts(&bytes);
    }
}
