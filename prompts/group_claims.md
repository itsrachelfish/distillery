Group the claims extracted from one file of a project's Markdown documentation into compound claims, and record how claims relate. Reply as JSON in the required format.

Each message holds the file path and a numbered list of claims. Each line has the claim's number, its kind in square brackets, its note, and after `|` the source text the claim rests on. Work from the list alone.

## Compound claims

- **One subject per compound claim.** Claims about the same subject belong together: a fact with its reasons, a setting with its values, the steps of one procedure.
- **Two or more parts.** Each claim is part of at most one compound claim. Claims that stand alone stay out.
- **A note, not a sentence:** the subject, a colon, then what the parts say together in the fewest words. Shorter than its parts combined. Names, numbers and code stay as written.
- **Only what the parts say.** No outside knowledge, and no inference past the claims.
- **The kind** that fits the parts together: behavior, configuration, requirement, decision, instruction or rationale.

## Relations

Relations hold between two claims of the list. Most lists have few, and none is a valid answer.

- **duplicates:** both claims state the same fact.
- **conflicts:** the claims can't both hold.
- **depends_on:** the first claim holds only while the second does.

## Reply

Keys are single letters, to keep replies short:

- `g`: the list of compound claims.
- `t`: the compound claim's text, as a note.
- `k`: in a compound claim, the kind. In a relation, the relation.
- `p`: the numbers of the parts.
- `r`: the list of relations.
- `a` and `b`: the numbers of a relation's first and second claim.
