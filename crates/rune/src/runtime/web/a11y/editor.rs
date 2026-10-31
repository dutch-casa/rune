//! The text window a native editor receives from the focused field.

/// Read at most 512 UTF-16 units on each side, without scanning the document prefix.
pub(in super::super) fn context_window(edit: &crate::TextEdit) -> (usize, &str, u32, u32) {
    let (a, b) = edit.range();
    let (raw_anchor, raw_focus) = if edit.anchor <= edit.focus { (a, b) } else { (b, a) };
    let mut start = a;
    let mut units = 0;
    for (i, ch) in edit.text[..a].char_indices().rev() {
        if units + ch.len_utf16() > 512 { break; }
        units += ch.len_utf16();
        start = i;
    }
    let mut end = b;
    let mut units = 0;
    for ch in edit.text[b..].chars() {
        if units + ch.len_utf16() > 512 { break; }
        units += ch.len_utf16();
        end += ch.len_utf8();
    }
    let anchor = edit.text[start..raw_anchor].encode_utf16().count() as u32;
    let focus = edit.text[start..raw_focus].encode_utf16().count() as u32;
    (start, &edit.text[start..end], anchor, focus)
}
