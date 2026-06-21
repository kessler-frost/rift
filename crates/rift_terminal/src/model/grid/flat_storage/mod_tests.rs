use itertools::Itertools;
use testing::{assert_rows_equal, ToRows as _};

use super::*;
use crate::model::char_or_str::CharOrStr;
use crate::model::grid::cell::{Cell, Flags};

#[test]
fn test_row_iteration() {
    let storage = FlatStorage::from_content_using_rows("hello world\n", 7, Some(2));

    let mut rows = storage.rows_from(0);

    let row1 = rows
        .next()
        .expect("should be able to get first row from storage");
    assert_eq!(row1.occ, 7);
    assert_eq!(row1[0].c, 'h');
    assert_eq!(row1[6].c, 'w');

    let row2 = rows
        .next()
        .expect("should be able to get first row from storage");
    assert_eq!(row2.occ, 4);
    assert_eq!(row2[0].c, 'o');
    assert_eq!(row2[3].c, 'd');

    assert!(rows.next().is_none());
}

#[test]
fn test_row_with_double_width_char() {
    let storage = FlatStorage::from_content_using_rows("hi 😀 hello\n", 6, Some(2));

    let mut rows = storage.rows_from(0);

    let row1 = rows
        .next()
        .expect("should be able to get first row from storage");
    assert_eq!(row1.occ, 6);
    assert_eq!(row1[0].c, 'h');
    assert_eq!(row1[3].c, '😀');
    assert!(row1[4].flags().contains(Flags::WIDE_CHAR_SPACER));
    assert_eq!(row1[5].c, ' ');

    let row2 = rows
        .next()
        .expect("should be able to get first row from storage");
    assert_eq!(row2.occ, 5);
    assert_eq!(row2[0].c, 'h');

    assert!(rows.next().is_none());
}

/// This test validates our handling of complex emoji sequences.
///
/// The three graphemes here are comprised of a number of Unicode characters.
/// Below are the individual characters that comprise the test string, with
/// "---" denoting how the string gets segmented into graphemes.
///
///  1. 🧑  1F9D1   ADULT
///  2.     1F3FF   EMOJI MODIFIER FITZPATRICK TYPE-6
///  3. ‍    200D    ZERO WIDTH JOINER
///  4. 🦰  1F9B0   EMOJI COMPONENT RED HAIR
///  ---
///  1. 👩  1F469   WOMAN
///  2. ‍    200D    ZERO WIDTH JOINER
///  3. 🦲  1F9B2   EMOJI COMPONENT BALD
///  ---
///  1. 🧔  1F9D4   BEARDED PERSON
///  2. 🏿   1F3FF   EMOJI MODIFIER FITZPATRICK TYPE-6
///  3. ‍    200D    ZERO WIDTH JOINER
///  4. ♂   2642    MALE SIGN
///  5. ️    FE0F    VARIATION SELECTOR-16
#[test]
#[ignore = "will not pass until using a version of unicode-width that includes commit afab363"]
fn test_row_with_complex_emoji() {
    let storage = FlatStorage::from_content_using_rows("🧑🏿‍🦰👩‍🦲🧔🏿‍♂️", 6, Some(1));

    let mut rows = storage.rows_from(0);
    let row1 = rows
        .next()
        .expect("should be able to get first row from storage");
    assert_eq!(row1.occ, 6);

    assert_eq!(row1[0].c, '🧑');
    assert!(matches!(
        row1[0].content_for_display(),
        CharOrStr::Str("🧑🏿‍🦰")
    ));

    assert!(row1[1].flags().contains(Flags::WIDE_CHAR_SPACER));
}

#[test]
fn test_push_rows_with_color() {
    let mut storage = FlatStorage::new(5, None, Some(2));

    let mut fg_cell = Cell::default();
    fg_cell.c = 'f';

    let mut red_cell = Cell::default();
    red_cell.c = 'r';
    red_cell.fg = ansi::Color::Named(ansi::NamedColor::Red);

    let row = Row::from_vec(
        vec![
            Cell::default(),
            Cell::default(),
            red_cell.clone(),
            red_cell,
            Cell::default(),
        ],
        5,
    );
    storage.push_rows([&row]);

    assert_eq!(storage.rows_from(0).next().unwrap().as_ref(), &row);
}

#[test]
fn test_push_rows_with_color_and_multibyte_chars() {
    let mut storage = FlatStorage::new(5, None, Some(2));

    let mut fg_cell = Cell::default();
    fg_cell.c = '❤';

    let mut red_cell = Cell::default();
    red_cell.c = 'r';
    red_cell.fg = ansi::Color::Named(ansi::NamedColor::Red);

    let row = Row::from_vec(
        vec![
            fg_cell.clone(),
            fg_cell.clone(),
            red_cell.clone(),
            red_cell,
            fg_cell,
        ],
        5,
    );
    storage.push_rows([&row]);

    assert_eq!(storage.rows_from(0).next().unwrap().as_ref(), &row);
}

#[test]
fn test_row_roundtrip_and_resize() {
    let num_cols = 5;
    let rows = "😀😃😄ag\na😁😆~!!\n😅sdf😂\n".to_rows(num_cols);

    // Build FlatStorage from the set of rows.
    let mut storage = FlatStorage::new(num_cols, None, None);
    storage.push_rows(&rows);

    // Make sure the generated rows match the original input.
    let flat_rows = storage
        .rows_from(0)
        .map(|row| row.as_ref().clone())
        .collect_vec();

    assert_rows_equal(&flat_rows, &rows);

    // "Resize" the storage, keeping the number of columns the same.
    storage.set_columns(num_cols);

    // Make sure the generated rows match the original input.
    let flat_rows = storage
        .rows_from(0)
        .map(|row| row.as_ref().clone())
        .collect_vec();

    assert_rows_equal(&flat_rows, &rows);
}

#[test]
fn test_styling_change_within_trailing_empty_cells() {
    let num_cols = 5;
    let mut rows = "a\nb\n".to_rows(num_cols);

    // Make the final cell in the first row bold.
    rows[0][num_cols - 1].flags.insert(Flags::BOLD);

    // Push the rows into storage.  This should produce a first row that is 5
    // cells long (the "a" followed by 3 empty cells followed by a bold empty
    // cell) and then clear the bold styling on the first cell of the second
    // line.
    let mut storage = FlatStorage::new(num_cols, None, None);
    storage.push_rows(&rows);

    let flat_rows = storage
        .rows_from(0)
        .map(|row| row.as_ref().clone())
        .collect_vec();

    // The first row's content should be 5 characters + a trailing newline.
    assert_eq!(flat_rows[0][0].c, 'a');
    assert_eq!(flat_rows[0][1].c, '\0');
    assert_eq!(flat_rows[0][2].c, '\0');
    assert_eq!(flat_rows[0][3].c, '\0');
    assert_eq!(flat_rows[0][4].c, '\0');
    assert!(!flat_rows[0][4].flags.contains(Flags::WRAPLINE));

    // The final cell in the first row should be bold, but the first cell in
    // the second row should not.
    assert!(flat_rows[0][num_cols - 1].flags.intersects(Flags::BOLD));
    assert!(!flat_rows[1][0].flags.intersects(Flags::BOLD));
}

#[test]
fn test_clear_after_truncate_front() {
    let num_cols = 20;
    let rows = "abcd\n789\n1 overflow\n2 overflow\n".to_rows(num_cols);

    let mut storage = FlatStorage::new(num_cols, Some(2), None);
    storage.push_rows(&rows);

    // We pushed 4 rows, and the limit is 2, so we should have truncated 2 rows.
    assert_eq!(storage.total_rows(), 2);
    assert_eq!(storage.num_truncated_rows(), 2);

    // Make sure the truncated rows are what we expect.
    assert_eq!(
        storage.rows_from(0).next().expect("should have a row")[0].c,
        '1'
    );
    assert_eq!(
        storage.rows_from(1).next().expect("should have a row")[0].c,
        '2'
    );

    // Clear flat storage, and ensure the state is as we expect.
    storage.clear();
    assert_eq!(storage.total_rows(), 0);
    // Should still have 2 truncated rows, as clearing storage doesn't affect
    // the number of rows we've truncated in total so far.
    assert_eq!(storage.num_truncated_rows(), 2);

    // Make sure we can push new rows.
    storage.push_rows(&rows);
    assert_eq!(storage.total_rows(), 2);
    assert_eq!(storage.num_truncated_rows(), 4);

    // Make sure remaining truncated rows are what we expect.
    assert_eq!(
        storage.rows_from(0).next().expect("should have a row")[0].c,
        '1'
    );
    assert_eq!(
        storage.rows_from(1).next().expect("should have a row")[0].c,
        '2'
    );
}

#[test]
fn test_new_with_zero_columns_does_not_panic() {
    // A grid can never legitimately have zero columns (the app layer clamps to
    // MIN_COLUMNS), but FlatStorage is a public library type that previously
    // panicked ("assertion failed: usizes >= 1" in Row::new, which in release
    // builds would instead write past a zero-capacity Vec) when constructed
    // with zero columns and then pushed into.  It must degrade gracefully.
    let mut storage = FlatStorage::new(0, None, None);
    storage.push_rows_from_string("hi\n");

    // We should still be able to materialize the row back out without panicking.
    let row = storage
        .rows_from(0)
        .next()
        .expect("should materialize a row from zero-column storage");
    assert_eq!(row[0].c, 'h');
}

#[test]
fn test_set_columns_to_zero_does_not_panic() {
    // Resizing storage down to zero columns must not panic when rows are later
    // materialized (RowIterator::new builds a Row of `columns` width, and
    // Row::new(0) is invalid).
    let mut storage = FlatStorage::new(5, None, None);
    storage.push_rows_from_string("hello\n");

    storage.set_columns(0);

    let row = storage
        .rows_from(0)
        .next()
        .expect("should materialize a row after resizing to zero columns");
    assert_eq!(row[0].c, 'h');
}

#[test]
fn test_clear_after_truncate_front_then_resize_and_push_does_not_panic() {
    let old_cols = 20;
    let new_cols = 21;
    let initial_content = "abcdefghijklmnopqrst\n".repeat(100);
    let rows = initial_content.as_str().to_rows(old_cols);

    let mut storage = FlatStorage::new(old_cols, Some(1), None);
    storage.push_rows(&rows);
    assert_eq!(storage.total_rows(), 1);

    storage.clear();
    storage.set_columns(new_cols);

    let new_rows = "new output\n".to_rows(new_cols);
    storage.push_rows(&new_rows);

    let row = storage
        .rows_from(0)
        .next()
        .expect("should materialize a row after clearing and resizing storage");
    assert_eq!(row[0].c, 'n');
}

/// Collects the occupied content of every row in storage as `(text, wraps)`
/// pairs, where `wraps` is true if the row soft-wraps into the next one.
fn rows_as_text(storage: &FlatStorage) -> Vec<(String, bool)> {
    storage
        .rows_from(0)
        .enumerate()
        .map(|(i, row)| {
            let text: String = (0..row.occ).map(|c| row[c].c).collect();
            (text, storage.row_wraps(i))
        })
        .collect()
}

#[test]
fn test_reflow_widening_unwraps_soft_wrapped_content() {
    // "abcdefgh" soft-wrapped at width 3 occupies three rows.
    let mut storage = FlatStorage::new(3, None, None);
    storage.push_rows_from_string("abcdefgh\n");
    assert_eq!(
        rows_as_text(&storage),
        vec![
            ("abc".to_string(), true),
            ("def".to_string(), true),
            ("gh".to_string(), false),
        ]
    );

    // Widening to 5 columns reflows the same content into two rows.
    storage.set_columns(5);
    assert_eq!(storage.total_rows(), 2);
    assert_eq!(
        rows_as_text(&storage),
        vec![("abcde".to_string(), true), ("fgh".to_string(), false)]
    );
}

#[test]
fn test_reflow_narrowing_rewraps_content() {
    let mut storage = FlatStorage::new(5, None, None);
    storage.push_rows_from_string("abcdefgh\n");
    assert_eq!(storage.total_rows(), 2);

    // Narrowing to 2 columns re-wraps the content into four rows, with only the
    // last row hard-wrapping (it ends in the trailing newline).
    storage.set_columns(2);
    assert_eq!(storage.total_rows(), 4);
    assert_eq!(
        rows_as_text(&storage),
        vec![
            ("ab".to_string(), true),
            ("cd".to_string(), true),
            ("ef".to_string(), true),
            ("gh".to_string(), false),
        ]
    );
}

#[test]
fn test_reflow_to_huge_width_keeps_content_on_one_row() {
    let mut storage = FlatStorage::new(4, None, None);
    storage.push_rows_from_string("abcdefgh\n");
    assert_eq!(storage.total_rows(), 2);

    // Resizing to a width far larger than the content collapses the soft-wrapped
    // rows back onto a single row without panicking or losing content.
    storage.set_columns(100_000);
    assert_eq!(storage.total_rows(), 1);
    assert_eq!(
        rows_as_text(&storage),
        vec![("abcdefgh".to_string(), false)]
    );
}

#[test]
fn test_reflow_round_trips_through_many_widths() {
    // Repeatedly reflowing the same content across a range of widths (including
    // narrowing to a single column and back out) must never panic and must
    // always preserve the content when returned to the original width.
    let mut storage = FlatStorage::new(7, None, None);
    storage.push_rows_from_string("the quick brown fox\n");
    let original = rows_as_text(&storage);

    for width in [1usize, 2, 3, 5, 8, 13, 21, 40, 7] {
        storage.set_columns(width);
        // The flattened content (ignoring wrap boundaries) must be stable.
        let flattened: String = rows_as_text(&storage)
            .into_iter()
            .map(|(text, _)| text)
            .collect();
        assert_eq!(flattened, "the quick brown fox");
    }

    // Back at width 7, we should have exactly the rows we started with.
    assert_eq!(rows_as_text(&storage), original);
}

#[test]
fn test_reflow_wide_chars_across_widths() {
    // Three double-width emoji.  Reflowing them across widths exercises the
    // leading-wide-char-spacer path (a wide char that won't fit in the single
    // remaining cell of a row wraps to the next row).
    let mut storage = FlatStorage::new(6, None, None);
    storage.push_rows_from_string("😀😀😀\n");

    // Helper: does any cell in the given row carry a leading-wide-char spacer?
    let has_leading_spacer = |storage: &FlatStorage, row_idx: usize| {
        let row = storage
            .rows_from(row_idx)
            .next()
            .expect("row should exist");
        (0..row.occ).any(|c| {
            row[c]
                .flags()
                .contains(Flags::LEADING_WIDE_CHAR_SPACER)
        })
    };

    // Width 6: all three emoji fit on a single row.
    assert_eq!(storage.total_rows(), 1);
    assert!(!has_leading_spacer(&storage, 0));

    // Width 5: two emoji occupy 4 cells; the third wide char can't fit in the
    // single remaining cell, so the row ends with a leading-wide-char spacer
    // and the third emoji moves to its own row.
    storage.set_columns(5);
    assert_eq!(storage.total_rows(), 2);
    assert!(has_leading_spacer(&storage, 0));
    assert!(!has_leading_spacer(&storage, 1));

    // Width 3: each emoji gets its own row (only one wide char fits per row).
    // The two non-final rows each end with a single empty cell that the next
    // row's wide char could not fit into, so they carry a leading-wide-char
    // spacer; the final row hard-wraps and does not.
    storage.set_columns(3);
    assert_eq!(storage.total_rows(), 3);
    for row_idx in 0..3 {
        let row = storage.rows_from(row_idx).next().expect("row should exist");
        assert_eq!(row[0].c, '😀');
    }
    assert!(has_leading_spacer(&storage, 0));
    assert!(has_leading_spacer(&storage, 1));
    assert!(!has_leading_spacer(&storage, 2));

    // Width 4: two emoji per row (4 cells each), so two rows with no leading
    // spacer.
    storage.set_columns(4);
    assert_eq!(storage.total_rows(), 2);
    assert!(!has_leading_spacer(&storage, 0));
    assert!(!has_leading_spacer(&storage, 1));
}
