//! Sections: the pieces a Markdown file is cut into, each sent to the model
//! on its own.
//!
//! A section runs from one heading to the next heading of any level, so no
//! text belongs to two sections. Parent headings are kept as a heading path,
//! which gives the model the context a section had in its file. Text before
//! the first heading is a section with no heading.
//!
//! Sections are computed from the file at a commit whenever they are needed,
//! and not stored: the same file contents always give the same sections.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// One section of a Markdown file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// The first line, counting from 1: the heading's line, when there is
    /// one.
    pub start_line: usize,
    /// The last line with text on it.
    pub end_line: usize,
    /// The headings above this section and its own, outermost first. Empty
    /// for text before the first heading.
    pub path: Vec<String>,
}

impl Section {
    /// The heading path as one line, such as `Server > Installation`.
    pub fn title(&self) -> String {
        if self.path.is_empty() {
            "Before the first heading".to_owned()
        } else {
            self.path.join(" > ")
        }
    }
}

/// A heading that starts a section.
struct Heading {
    line: usize,
    level: usize,
    title: String,
}

/// Cuts Markdown text into sections, in file order.
pub fn split(text: &str) -> Vec<Section> {
    let lines: Vec<&str> = text.lines().collect();
    let headings = headings(text);

    let mut sections = Vec::new();
    // Headings above the current one: their levels and titles.
    let mut parents: Vec<(usize, String)> = Vec::new();
    let mut add = |start: usize, next_start: usize, path: Vec<String>| {
        // Blank lines before the next heading belong to no section.
        let mut end = next_start - 1;
        while end >= start && lines[end - 1].trim().is_empty() {
            end -= 1;
        }
        if end >= start {
            sections.push(Section {
                start_line: start,
                end_line: end,
                path,
            });
        }
    };

    let first_heading = headings.first().map_or(lines.len() + 1, |h| h.line);
    add(1, first_heading, Vec::new());
    for (index, heading) in headings.iter().enumerate() {
        while parents
            .last()
            .is_some_and(|(level, _)| *level >= heading.level)
        {
            parents.pop();
        }
        parents.push((heading.level, heading.title.clone()));
        let path = parents.iter().map(|(_, title)| title.clone()).collect();
        let next_start = headings
            .get(index + 1)
            .map_or(lines.len() + 1, |next| next.line);
        add(heading.line, next_start, path);
    }
    sections
}

/// The headings that start sections. Headings inside lists and block quotes
/// don't, so quoted examples stay in one piece.
fn headings(text: &str) -> Vec<Heading> {
    // Front matter between `---` lines is metadata. Without this option,
    // its last line reads as a heading underlined by the closing `---`.
    let options = Options::ENABLE_YAML_STYLE_METADATA_BLOCKS;
    let mut found = Vec::new();
    // How many elements the parser is inside of.
    let mut depth = 0;
    // The heading being read, when it starts a section.
    let mut open: Option<Heading> = None;

    for (event, range) in Parser::new_ext(text, options).into_offset_iter() {
        match event {
            Event::Start(tag) => {
                if depth == 0
                    && let Tag::Heading { level, .. } = tag
                {
                    open = Some(Heading {
                        line: line_of(text, range.start),
                        level: level_number(level),
                        title: String::new(),
                    });
                }
                depth += 1;
            }
            Event::End(tag) => {
                depth -= 1;
                if depth == 0
                    && let TagEnd::Heading(_) = tag
                    && let Some(mut heading) = open.take()
                {
                    heading.title = heading.title.trim().to_owned();
                    found.push(heading);
                }
            }
            Event::Text(part) | Event::Code(part) => {
                if let Some(heading) = open.as_mut() {
                    heading.title.push_str(&part);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(heading) = open.as_mut() {
                    heading.title.push(' ');
                }
            }
            _ => {}
        }
    }
    found
}

/// The line a byte offset is on, counting from 1.
fn line_of(text: &str, offset: usize) -> usize {
    text.as_bytes()[..offset]
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count()
        + 1
}

fn level_number(level: HeadingLevel) -> usize {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}
