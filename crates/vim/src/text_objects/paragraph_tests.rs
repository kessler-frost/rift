use super::*;

#[test]
fn vim_inner_paragraph_empty_buffer() {
    assert_eq!(vim_inner_paragraph("", 0), Some(0.into()..0.into()));
    assert_eq!(vim_inner_paragraph("", 1), None);
}

#[test]
fn vim_a_paragraph_empty_buffer() {
    assert_eq!(vim_a_paragraph("", 0), Some(0.into()..0.into()));
    assert_eq!(vim_a_paragraph("", 1), None);
}

#[test]
fn vim_inner_paragraph_single_paragraph() {
    let text = "foo bar\nnext line\n";

    for (i, ch) in text.chars().enumerate() {
        if ch != '\n' {
            let range = vim_inner_paragraph(text, i).unwrap();
            assert_eq!(range, 0.into()..text.len().into());
        }
    }
}

#[test]
fn vim_a_paragraph_single_paragraph() {
    let text = "foo bar\nnext line\n";

    for (i, ch) in text.chars().enumerate() {
        if ch != '\n' {
            let range = vim_a_paragraph(text, i).unwrap();
            assert_eq!(range, 0.into()..text.len().into());
        }
    }
}

#[test]
fn vim_inner_paragraph_two_paragraphs() {
    let text = "first line\nof first para\n\nsecond para line\n";
    let blank_index = text.find("\n\n").unwrap();
    let second_start = blank_index + 2;

    for (i, ch) in text.chars().take(blank_index).enumerate() {
        if ch != '\n' {
            let range = vim_inner_paragraph(text, i).unwrap();
            assert_eq!(range, 0.into()..blank_index.into());
        }
    }

    for (i, ch) in text.chars().enumerate().skip(second_start) {
        if ch != '\n' {
            let range = vim_inner_paragraph(text, i).unwrap();
            assert_eq!(range, second_start.into()..text.len().into());
        }
    }
}

#[test]
fn vim_inner_paragraph_three_paragraphs() {
    let text = "first\n\nsecond\n\nthird\n";
    let (first_blank, second_blank) = {
        let mut it = text.match_indices("\n\n");
        (it.next().unwrap().0, it.next().unwrap().0)
    };
    let second_start = first_blank + 2;
    let third_start = second_blank + 2;

    for (i, ch) in text.chars().take(first_blank).enumerate() {
        if ch != '\n' {
            let range = vim_inner_paragraph(text, i).unwrap();
            assert_eq!(range, 0.into()..first_blank.into());
        }
    }

    for (i, ch) in text
        .chars()
        .enumerate()
        .skip(second_start)
        .take(second_blank - second_start)
    {
        if ch != '\n' {
            let range = vim_inner_paragraph(text, i).unwrap();
            assert_eq!(range, second_start.into()..second_blank.into());
        }
    }

    for (i, ch) in text.chars().enumerate().skip(third_start) {
        if ch != '\n' {
            let range = vim_inner_paragraph(text, i).unwrap();
            assert_eq!(range, third_start.into()..text.len().into());
        }
    }
}

#[test]
fn vim_a_paragraph_two_paragraphs() {
    let text = "first line\nof first para\n\nsecond para line\n";
    let blank_index = text.find("\n\n").unwrap();

    for (i, ch) in text.chars().take(blank_index).enumerate() {
        if ch != '\n' {
            let range = vim_a_paragraph(text, i).unwrap();
            assert_eq!(range, 0.into()..(blank_index + 1).into());
        }
    }

    let second_range_start = (blank_index + 1).into();
    let second_range_end = text.len().into();
    let second_start = blank_index + 2;

    for (i, ch) in text.chars().enumerate().skip(second_start) {
        if ch != '\n' {
            let range = vim_a_paragraph(text, i).unwrap();
            assert_eq!(range, second_range_start..second_range_end);
        }
    }
}

#[test]
fn vim_a_paragraph_three_paragraphs() {
    let text = "first\n\nsecond\n\nthird\n";
    let (first_blank, second_blank) = {
        let mut it = text.match_indices("\n\n");
        (it.next().unwrap().0, it.next().unwrap().0)
    };
    let second_start = first_blank + 2;
    let third_start = second_blank + 2;

    for (i, ch) in text.chars().take(first_blank).enumerate() {
        if ch != '\n' {
            let range = vim_a_paragraph(text, i).unwrap();
            assert_eq!(range, 0.into()..(first_blank + 1).into());
        }
    }

    for (i, ch) in text
        .chars()
        .enumerate()
        .skip(second_start)
        .take(second_blank - second_start)
    {
        if ch != '\n' {
            let range = vim_a_paragraph(text, i).unwrap();
            assert_eq!(range, second_start.into()..(second_blank + 1).into());
        }
    }

    let last_range = (second_blank + 1).into()..text.len().into();
    for (i, ch) in text.chars().enumerate().skip(third_start) {
        if ch != '\n' {
            let range = vim_a_paragraph(text, i).unwrap();
            assert_eq!(range, last_range);
        }
    }
}

#[test]
fn vim_inner_paragraph_cursor_on_leading_newline_does_not_panic() {
    // Cursor on a newline at offset 0 (buffer begins with a blank line). Previously this
    // computed `offset - 1` before adding the newline count, underflowing `CharOffset` and
    // panicking (debug) / wrapping (release). The result must be a valid, non-inverted range.
    let range = vim_inner_paragraph("\n", 0).unwrap();
    assert!(
        range.start <= range.end,
        "range must not be inverted: {range:?}"
    );
    assert_eq!(range, 0.into()..0.into());

    // A multi-newline run that begins at offset 0 must also be handled without underflow.
    // Consistent with the interior blank-line case (one boundary newline trimmed from each
    // side of the run): the run spans offsets 0..=2, trimmed inner is 1..2.
    let range = vim_inner_paragraph("\n\n\n", 0).unwrap();
    assert!(
        range.start <= range.end,
        "range must not be inverted: {range:?}"
    );
    assert_eq!(range, 1.into()..2.into());
}

#[test]
fn vim_a_paragraph_cursor_on_leading_newline_does_not_panic() {
    let range = vim_a_paragraph("\n", 0).unwrap();
    assert!(
        range.start <= range.end,
        "range must not be inverted: {range:?}"
    );

    let range = vim_a_paragraph("\n\n\n", 0).unwrap();
    assert!(
        range.start <= range.end,
        "range must not be inverted: {range:?}"
    );
}

#[test]
fn vim_inner_paragraph_blank_lines() {
    let text = "first\n\n\nsecond\n";

    for offset in 5..=7 {
        let range = vim_inner_paragraph(text, offset).unwrap();
        assert_eq!(range, 6.into()..7.into());
    }
}

#[test]
fn vim_a_paragraph_blank_lines() {
    let text = "first\n\n\nsecond\n";

    for offset in 5..=7 {
        let range = vim_a_paragraph(text, offset).unwrap();
        assert_eq!(range, 6.into()..text.len().into());
    }
}

#[test]
fn vim_a_paragraph_many_trailing_blank_lines() {
    let text = "first\n\n\nsecond\n\n\n\n\nthird";

    for offset in 0..=4 {
        let range = vim_a_paragraph(text, offset).unwrap();
        assert_eq!(range, 0.into()..7.into());
    }

    for offset in 8..=13 {
        let range = vim_a_paragraph(text, offset).unwrap();
        assert_eq!(range, 8.into()..18.into());
    }

    for offset in 19..=23 {
        let range = vim_a_paragraph(text, offset).unwrap();
        assert_eq!(range, 15.into()..24.into());
    }
}

#[test]
fn vim_paragraph_lines_with_spaces_included() {
    // Despite being invisible, a line containing spaces still counts as "content".
    let text = "first\n\n    \nsecond";

    let range = vim_a_paragraph(text, 3).unwrap();
    assert_eq!(range, 0.into()..6.into());
    let range = vim_a_paragraph(text, 14).unwrap();
    assert_eq!(range, 6.into()..18.into());
}
