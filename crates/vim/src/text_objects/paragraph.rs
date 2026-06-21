use std::ops::Range;

use riftui_core::text::TextBuffer;
use string_offset::CharOffset;

use crate::{find_next_paragraph_end, find_previous_paragraph_start};

/// Vim's "inner paragraph" text object, e.g. `dip`. This includes lines surrounding the cursor
/// until blank lines are encountered in either direction, not including either blank line.
/// See https://vimdoc.sourceforge.net/htmldoc/motion.html#ip
/// or enter ":help ip" in Vim.
///
/// Returns None when the buffer is empty or the offset is out of bounds.
pub fn vim_inner_paragraph<T, C>(buffer: &T, offset: C) -> Option<Range<CharOffset>>
where
    T: TextBuffer + ?Sized,
    C: Into<CharOffset>,
{
    let offset = offset.into();
    let mut chars = buffer.chars_at(offset).ok()?.peekable();
    if chars.peek().is_some_and(|c| *c == '\n') {
        // The cursor sits on a contiguous run of newlines. `take_while` counts at least one
        // (the newline under the cursor), so `offset + count - 1` is always >= offset and never
        // underflows. (The original `offset - 1 + count` evaluated the subtraction first, which
        // underflowed `CharOffset` when the run begins at offset 0.)
        let trailing_newlines = chars.take_while(|c| *c == '\n').count();
        let paragraph_end = offset + trailing_newlines - 1;
        // `leading_newlines` only counts newlines strictly before `offset`, so it is always
        // <= offset and `offset + 1 - leading_newlines` cannot underflow.
        let leading_newlines = buffer
            .chars_rev_at(offset)
            .ok()?
            .take_while(|c| *c == '\n')
            .count();
        let paragraph_start = offset + 1 - leading_newlines;
        // A single-newline run (e.g. a lone blank line at the start of the buffer) trims to an
        // empty range; clamp so the range is never inverted.
        let paragraph_start = paragraph_start.min(paragraph_end);
        return Some(paragraph_start..paragraph_end);
    }
    let paragraph_start = find_previous_paragraph_start(buffer, offset)
        .map(|i| i + 1)
        .unwrap_or_default();
    let paragraph_end = find_next_paragraph_end(buffer, offset)
        .map(|i| i - 1)
        .unwrap_or_else(|| offset + chars.count());

    Some(paragraph_start..paragraph_end)
}

/// Vim's "a paragraph" text object, e.g. `dap`. This includes lines surrounding the cursor until
/// blank lines are encountered in either direction, including one blank line. Usually the blank
/// line to be included is the one following, unless the paragraph is at the end of the buffer in
/// which case we include the one preceding.
/// See https://vimdoc.sourceforge.net/htmldoc/motion.html#ap
/// or enter ":help ap" in Vim.
///
/// Returns None when the buffer is empty or the offset is out of bounds.
pub fn vim_a_paragraph<T, C>(buffer: &T, offset: C) -> Option<Range<CharOffset>>
where
    T: TextBuffer + ?Sized,
    C: Into<CharOffset>,
{
    let offset = offset.into();
    let mut chars = buffer.chars_at(offset).ok()?.peekable();
    if chars.peek().is_some_and(|c| *c == '\n') {
        let paragraph_end = find_next_paragraph_end(buffer, offset)
            .map(|i| i - 1)
            .unwrap_or_else(|| offset + chars.count());
        let paragraph_start = offset + 1
            - buffer
                .chars_rev_at(offset)
                .ok()?
                .take_while(|c| *c == '\n')
                .count();
        return Some(paragraph_start..paragraph_end);
    }
    let end_offset = find_next_paragraph_end(buffer, offset);
    // We either need to include all the consecutive blank lines above or below the paragraph
    // content.
    let paragraph_start = find_previous_paragraph_start(buffer, offset)
        .map(|i| {
            // If end_offset is Some, we'll include the consecutive blank lines below.
            if end_offset.is_some() {
                i + 1
            } else {
                // Calculate the number of blank lines above the start of the paragraph content.
                i + 1
                    - buffer
                        .chars_rev_at(i)
                        .map(|chars| chars.take_while(|c| *c == '\n').count())
                        .unwrap_or_default()
            }
        })
        .unwrap_or_default();
    let paragraph_end = end_offset
        .map(|i| {
            // Calculate the number of blank lines below the end of the paragraph content.
            i - 1
                + buffer
                    .chars_at(i)
                    .map(|chars| chars.take_while(|c| *c == '\n').count())
                    .unwrap_or_default()
        })
        .unwrap_or_else(|| offset + chars.count());

    Some(paragraph_start..paragraph_end)
}

#[cfg(test)]
#[path = "paragraph_tests.rs"]
mod tests;
