//! Finding quotes in source text.
//!
//! Models copy quotes with small differences in spacing and line breaks, so
//! quotes are matched ignoring whitespace. A quote that matches gives the
//! lines it covers, which is where its claim points.

/// The first and last line of `quote` in `lines`, where the first of
/// `lines` has the line number `first_line`. `None` when the quote isn't
/// there, or holds nothing but whitespace. Where a quote appears more than
/// once, the first place counts.
pub fn find(lines: &[&str], first_line: usize, quote: &str) -> Option<(usize, usize)> {
    let wanted: String = quote.chars().filter(|c| !c.is_whitespace()).collect();
    if wanted.is_empty() {
        return None;
    }
    // The text without whitespace, and the line each of its bytes came from.
    let mut text = String::new();
    let mut line_of = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        for c in line.chars().filter(|c| !c.is_whitespace()) {
            text.push(c);
            line_of.resize(text.len(), first_line + index);
        }
    }
    let start = text.find(&wanted)?;
    Some((line_of[start], line_of[start + wanted.len() - 1]))
}
