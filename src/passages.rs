//! Passages: the numbered pieces of a section that claims point to.
//!
//! Models copy numbers reliably and count characters badly. So Distillery
//! cuts each section into passages, one per sentence, and marks each with
//! its number in the text the model reads. Claims name passages by number,
//! and Distillery, which knows where every passage sits, supplies the lines,
//! the columns and the quoted text itself.

/// One sentence, or one line without sentence ends, of a section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Passage {
    /// The line number in the file, from 1.
    pub line: usize,
    /// The columns of the first and last character in the line, from 1,
    /// counted in characters.
    pub start_column: usize,
    pub end_column: usize,
}

/// Cuts lines into passages, where the first of `lines` has the line number
/// `first_line`. Lines are cut after `.`, `!` or `?` followed by a space;
/// blank lines hold no passage. Cuts in the wrong place, such as after an
/// abbreviation, only make passages smaller.
pub fn cut(lines: &[&str], first_line: usize) -> Vec<Passage> {
    let mut passages = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let chars: Vec<char> = line.chars().collect();
        let mut start = None;
        for (at, c) in chars.iter().enumerate() {
            if start.is_none() && !c.is_whitespace() {
                start = Some(at);
            }
            let ends_sentence = matches!(c, '.' | '!' | '?')
                && chars.get(at + 1).is_some_and(|next| next.is_whitespace());
            if let (Some(from), true) = (start, ends_sentence || at + 1 == chars.len())
                && !c.is_whitespace()
            {
                passages.push(Passage {
                    line: first_line + index,
                    start_column: from + 1,
                    end_column: at + 1,
                });
                start = None;
            }
        }
        // A line ending in whitespace closes its last passage at the last
        // character that isn't.
        if let Some(from) = start
            && let Some(last) = chars.iter().rposition(|c| !c.is_whitespace())
        {
            passages.push(Passage {
                line: first_line + index,
                start_column: from + 1,
                end_column: last + 1,
            });
        }
    }
    passages
}

/// The lines as the model reads them: every passage has its number in
/// square brackets before it, counted from 1. Indentation and blank lines
/// stay, so lists and code keep their shape.
pub fn numbered(lines: &[&str], first_line: usize, passages: &[Passage]) -> String {
    let mut text = String::new();
    let mut number = 0;
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            text.push('\n');
        }
        let chars: Vec<char> = line.chars().collect();
        let mut written = 0;
        for passage in passages
            .iter()
            .filter(|passage| passage.line == first_line + index)
        {
            number += 1;
            text.extend(&chars[written..passage.start_column - 1]);
            text.push_str(&format!("[{number}] "));
            text.extend(&chars[passage.start_column - 1..passage.end_column]);
            written = passage.end_column;
        }
        text.extend(&chars[written..]);
    }
    text
}

/// The source text from the start of one passage to the end of another,
/// exactly as written.
pub fn quote(lines: &[&str], first_line: usize, from: &Passage, to: &Passage) -> String {
    let mut text = String::new();
    for line in from.line..=to.line {
        let chars: Vec<char> = lines[line - first_line].chars().collect();
        let start = if line == from.line {
            from.start_column - 1
        } else {
            0
        };
        let end = if line == to.line {
            to.end_column
        } else {
            chars.len()
        };
        if line > from.line {
            text.push('\n');
        }
        text.extend(&chars[start..end]);
    }
    text
}
