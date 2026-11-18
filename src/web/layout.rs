//! The page frame shared by every page, and small helpers for showing
//! values.

use maud::{DOCTYPE, Markup, html};

use super::Flash;

/// How many characters of a commit hash pages show.
const SHORT_HASH: usize = 12;

/// A complete page with the site navigation and any notice or error.
pub fn page(title: &str, flash: &Flash, body: Markup) -> Markup {
    frame(title, flash, body, None)
}

/// A page that reloads itself every `seconds`, to follow a job under way
/// with no script.
pub fn reloading_page(title: &str, flash: &Flash, body: Markup, seconds: u32) -> Markup {
    frame(title, flash, body, Some(seconds))
}

fn frame(title: &str, flash: &Flash, body: Markup, reload: Option<u32>) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                @if let Some(seconds) = reload {
                    meta http-equiv="refresh" content=(seconds);
                }
                title { (title) " · Distillery" }
                link rel="stylesheet" href="/style.css";
            }
            body {
                header {
                    nav {
                        a.brand href="/" { "Distillery" }
                        a href="/" { "Sources" }
                        a href="/job" { "Job" }
                        a href="/usage" { "Usage" }
                    }
                }
                main {
                    // The Job page shows the job itself.
                    @if reload.is_none() && super::jobs::running() {
                        p.banner role="status" {
                            "A job is running. "
                            a href="/job" { "See how far it has come" }
                        }
                    }
                    @if let Some(notice) = &flash.notice {
                        p.flash.notice role="status" { (notice) }
                    }
                    @if let Some(error) = &flash.error {
                        p.flash.error role="alert" { (error) }
                    }
                    (body)
                }
            }
        }
    }
}

/// Encodes a value for use in a URL query.
pub fn url_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// A commit hash shortened for reading, with the full hash on hover.
pub fn commit(hash: &str) -> Markup {
    let short = hash.get(..SHORT_HASH).unwrap_or(hash);
    html! {
        code title=(hash) { (short) }
    }
}

/// A cost in US dollars, from millionths of a dollar. Costs under a dollar
/// show four decimals, since one call often costs less than a cent.
pub fn dollars(micros: i64) -> String {
    let dollars = micros as f64 / 1_000_000.0;
    if micros.abs() < 1_000_000 {
        format!("${dollars:.4}")
    } else {
        format!("${dollars:.2}")
    }
}

/// A count with thousands separators.
pub fn number(n: i64) -> String {
    let digits = crate::progress::thousands(n.unsigned_abs());
    if n < 0 { format!("-{digits}") } else { digits }
}

/// A time the database recorded, which SQLite keeps in UTC.
pub fn utc(time: &str) -> Markup {
    html! {
        time datetime=(format!("{}Z", time.replace(' ', "T"))) { (time) " UTC" }
    }
}
