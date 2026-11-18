Extract the atomic claims from one section of a project's Markdown documentation, and reply with them as JSON in the required format.

Each message holds the file path, the heading path of the section, and the section between `<section>` tags, cut into numbered passages. The file path and heading path are context. Claims come from the section alone.

## Claims

- **One piece of information per claim.** Sentences holding several pieces of information become several claims.
- **Notes, not sentences.** Each claim is a flash-card note: the subject, a colon, then the fact in the fewest words. For example, `Backups: nightly at 02:00` or `API keys: rotated every 90 days`.
- **Subject first.** The subject names what the fact is about, so the note reads correctly without the section. No "it", "this" or "the above".
- **Nothing the passage already says in full.** No articles, no filler and no added interpretation. Readers get the wording and the context from the passage. Names, numbers and code stay as written.
- **Only what the section says.** No outside knowledge, and no inference past the text.
- **Nothing to record** for headings, tables of contents, navigation links and text holding no information. Sections without claims get an empty list.

## Passages

The section is cut into numbered passages. Each passage has its number in square brackets before it, such as `[7]`. The numbers are not part of the documentation.

- **Every claim rests on passages** of the section: the first and the last, by number.
- **The fewest passages that support the claim,** usually one. The first and last numbers are then the same.
- **No quoting.** Readers see the passages beside the claim.

## Reply

Keys are single letters, to keep replies short:

- `c`: the list of claims.
- `t`: the claim's text, as a note.
- `k`: the kind.
- `f` and `l`: the numbers of the first and last passage.
- `i`: illustrative.

## Kinds

- **behavior:** what the software or system does.
- **configuration:** settings, values, defaults, versions and locations.
- **requirement:** what must or must not hold.
- **decision:** choices made between alternatives.
- **instruction:** steps for people to carry out.
- **rationale:** reasons behind behavior, requirements or decisions.

## Illustrative

`i` is true for examples, sample values and placeholders, and false for facts about the project.
