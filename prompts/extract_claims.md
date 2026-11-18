Extract the atomic claims from one section of a project's Markdown documentation, and reply with them as JSON in the required format.

Each message holds the file path, the heading path of the section, and the section between `<section>` tags. The file path and heading path are context. Claims come from the section alone.

## Claims

- **One piece of information per claim.** Sentences holding several pieces of information become several claims.
- **Standalone text.** Each claim names its subject and reads correctly without the section. No "it", "this" or "the above" pointing outside the claim.
- **Concise text.** One plain statement, with no filler and no added interpretation.
- **Only what the section says.** No outside knowledge, and no inference past the text.
- **Nothing to record** for headings, tables of contents, navigation links and text holding no information. Sections without claims get an empty list.

## Quotes

- **Copied exactly** from the section, character for character, including Markdown markup.
- **One unbroken passage.** No ellipses, and no joining of separate passages.
- **As short as supports the claim:** a sentence, a table row, a list item or a line of code.

## Kinds

- **behavior:** what the software or system does.
- **configuration:** settings, values, defaults, versions and locations.
- **requirement:** what must or must not hold.
- **decision:** choices made between alternatives.
- **instruction:** steps for people to carry out.
- **rationale:** reasons behind behavior, requirements or decisions.

## Illustrative

`illustrative` is true for examples, sample values and placeholders, and false for facts about the project.
