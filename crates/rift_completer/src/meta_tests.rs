use super::*;

/*
0 1 2 3
w a r p
-------
0     4  << the span for the string "rift" is (0, 4)

Spanned {
    item: String::new("rift"),  << rift string
    span: Span::new(0, 4)       << span
}

or >> String::new("rift").spanned(Span::new(0, 4))        */
fn rift() -> Spanned<String> {
    String::from("rift").spanned(Span::new(0, 4))
}

fn empty() -> Spanned<String> {
    String::new().spanned_unknown()
}

#[test]
fn knows_distances() {
    assert!(rift().span.distance() == 4);
    assert!(empty().span.distance() == 0);
}
