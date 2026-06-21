use super::*;

#[test]
fn location_ordering() {
    assert!(Point::new(0, 0) == Point::new(0, 0));
    assert!(Point::new(1, 0) > Point::new(0, 0));
    assert!(Point::new(0, 1) > Point::new(0, 0));
    assert!(Point::new(1, 1) > Point::new(0, 0));
    assert!(Point::new(1, 1) > Point::new(0, 1));
    assert!(Point::new(1, 1) > Point::new(1, 0));
}

#[test]
fn wrapping_sub() {
    let num_cols = 42;
    let point = Point::new(0, 13);

    let result = point.wrapping_sub(num_cols, 1);

    assert_eq!(result, Point::new(0, point.col - 1));
}

#[test]
fn wrapping_sub_wrap() {
    let num_cols = 42;
    let point = Point::new(1, 0);

    let result = point.wrapping_sub(num_cols, 1);

    assert_eq!(result, Point::new(0, num_cols - 1));
}

#[test]
fn wrapping_sub_clamp() {
    let num_cols = 42;
    let point = Point::new(0, 0);

    let result = point.wrapping_sub(num_cols, 1);

    assert_eq!(result, point);
}

#[test]
fn wrapping_add() {
    let num_cols = 42;
    let point = Point::new(0, 13);

    let result = point.wrapping_add(num_cols, 1);

    assert_eq!(result, Point::new(0, point.col + 1));
}

#[test]
fn wrapping_add_wrap() {
    let num_cols = 42;
    let point = Point::new(0, num_cols - 1);

    let result = point.wrapping_add(num_cols, 1);

    assert_eq!(result, Point::new(1, 0));
}

#[test]
fn add_absolute() {
    let point = Point::new(0, 13);

    let result = point.add_absolute(&(1, 42), Boundary::Clamp, 1);

    assert_eq!(result, Point::new(0, point.col + 1));
}

#[test]
fn add_absolute_wrapline() {
    let point = Point::new(1, 41);

    let result = point.add_absolute(&(2, 42), Boundary::Clamp, 1);

    assert_eq!(result, Point::new(0, 0));
}

#[test]
fn add_absolute_multiline_wrapline() {
    let point = Point::new(2, 9);

    let result = point.add_absolute(&(3, 10), Boundary::Clamp, 11);

    assert_eq!(result, Point::new(0, 0));
}

#[test]
fn add_absolute_clamp() {
    let point = Point::new(0, 41);

    let result = point.add_absolute(&(1, 42), Boundary::Clamp, 1);

    assert_eq!(result, point);
}

#[test]
fn add_absolute_wrap() {
    let point = Point::new(0, 41);

    let result = point.add_absolute(&(3, 42), Boundary::Wrap, 1);

    assert_eq!(result, Point::new(2, 0));
}

#[test]
fn add_absolute_multiline_wrap() {
    let point = Point::new(0, 9);

    let result = point.add_absolute(&(3, 10), Boundary::Wrap, 11);

    assert_eq!(result, Point::new(1, 0));
}

#[test]
fn sub_absolute() {
    let point = Point::new(0, 13);

    let result = point.sub_absolute(&(1, 42), Boundary::Clamp, 1);

    assert_eq!(result, Point::new(0, point.col - 1));
}

#[test]
fn sub_absolute_wrapline() {
    let point = Point::new(0, 0);

    let result = point.sub_absolute(&(2, 42), Boundary::Clamp, 1);

    assert_eq!(result, Point::new(1, 41));
}

#[test]
fn sub_absolute_multiline_wrapline() {
    let point = Point::new(0, 0);

    let result = point.sub_absolute(&(3, 10), Boundary::Clamp, 11);

    assert_eq!(result, Point::new(2, 9));
}

#[test]
fn sub_absolute_wrap() {
    let point = Point::new(2, 0);

    let result = point.sub_absolute(&(3, 42), Boundary::Wrap, 1);

    assert_eq!(result, Point::new(0, 41));
}

#[test]
fn sub_absolute_multiline_wrap() {
    let point = Point::new(2, 0);

    let result = point.sub_absolute(&(3, 10), Boundary::Wrap, 11);

    assert_eq!(result, Point::new(1, 9));
}

#[test]
fn test_point_difference() {
    let a = Point::new(3, 10);
    assert_eq!(a.distance(30, &a), 0);

    let b = Point::new(3, 6);
    assert_eq!(a.distance(30, &b), 4);
    assert_eq!(b.distance(30, &a), 4);

    let c = Point::new(4, 2);
    assert_eq!(a.distance(30, &c), 22);
    assert_eq!(c.distance(30, &a), 22);
}

// The following tests exercise degenerate `num_cols == 0` inputs.  A grid can
// never have zero columns in normal operation (the app layer clamps to
// `MIN_COLUMNS`), but these `Point` helpers are part of the crate's public API
// and previously panicked with "attempt to divide by zero" / subtract-overflow
// when handed a zero column count.  They must instead degrade gracefully so a
// stray zero-width caller (or a future call site that forgets to clamp) can't
// crash the terminal.

#[test]
fn wrapping_add_zero_cols_is_noop() {
    let point = Point::new(3, 7);
    assert_eq!(point.wrapping_add(0, 5), point);
}

#[test]
fn wrapping_sub_zero_cols_is_noop() {
    let point = Point::new(3, 7);
    assert_eq!(point.wrapping_sub(0, 5), point);
}

#[test]
fn add_absolute_zero_cols_is_noop() {
    let point = Point::new(2, 4);
    assert_eq!(point.add_absolute(&(5, 0), Boundary::Clamp, 3), point);
    assert_eq!(point.add_absolute(&(5, 0), Boundary::Wrap, 3), point);
}

#[test]
fn sub_absolute_zero_cols_is_noop() {
    let point = Point::new(2, 4);
    assert_eq!(point.sub_absolute(&(5, 0), Boundary::Clamp, 3), point);
    assert_eq!(point.sub_absolute(&(5, 0), Boundary::Wrap, 3), point);
}

#[test]
fn distance_zero_cols_does_not_panic() {
    let a = Point::new(1, 2);
    let b = Point::new(3, 4);
    // `distance` multiplies the row by `num_cols`; with zero columns the row
    // term vanishes and only the columns contribute.  This does not divide by
    // zero, so it is safe today -- this test just pins that behavior down so a
    // future refactor that introduces a division here gets caught.
    assert_eq!(a.distance(0, &b), 2);
    assert_eq!(b.distance(0, &a), 2);
}
