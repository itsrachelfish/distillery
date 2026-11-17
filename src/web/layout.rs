//! The page frame shared by every page, and small helpers for showing
//! values.

use maud::{DOCTYPE, Markup, html};

/// How many characters of a commit hash pages show.
const SHORT_HASH: usize = 12;

/// A complete page with the site navigation.
pub fn page(title: &str, body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " · Distillery" }
                link rel="stylesheet" href="/style.css";
            }
            body {
                header {
                    nav {
                        a.brand href="/" { "Distillery" }
                        a href="/" { "Sources" }
                    }
                }
                main {
                    (body)
                }
            }
        }
    }
}

/// A commit hash shortened for reading, with the full hash on hover.
pub fn commit(hash: &str) -> Markup {
    let short = hash.get(..SHORT_HASH).unwrap_or(hash);
    html! {
        code title=(hash) { (short) }
    }
}

/// A time the database recorded, which SQLite keeps in UTC.
pub fn utc(time: &str) -> Markup {
    html! {
        time datetime=(format!("{}Z", time.replace(' ', "T"))) { (time) " UTC" }
    }
}
