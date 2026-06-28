use fossil::core::models::transpose::{decode, encode};

fn roundtrip(d: &[u8]) {
    assert_eq!(decode(&encode(d), d.len()), d);
}

#[test]
fn roundtrips_csv() {
    roundtrip(b"a,b,c\n1,2,3\n4,5,6\n");
}

#[test]
fn roundtrips_no_trailing_newline() {}

#[test]
fn roundtrips_non_csv() {
    roundtrip(b"just some normal prose, with one comma");
}

#[test]
fn roundtrips_empty() {
    roundtrip(&[]);
}

#[test]
fn shrinks_repetitive_columns() {
    let mut d = String::from("name,status\n");
    for i in 0..400 {
        d.push_str(&format!("user{},active\n", i % 5));
    }
    assert!(encode(d.as_bytes()).len() < d.len());
}
