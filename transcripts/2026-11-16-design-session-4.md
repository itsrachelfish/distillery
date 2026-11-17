# Transcript: Distillery design session 4, 2026-11-16

Participants: Rachel Fish, Claude (Opus 5.5, Anthropic). Times are America/Denver (MST). Continues [session 3](2026-11-12-design-session-3.md). Tool calls are omitted. Message text is verbatim, except that citation markup is removed from Claude's messages and blank lines separate Rachel's one-line paragraphs.

Rachel's README edit between commits 1a56fe8 and ab109bf condensed it from 67 to 53 words. Her 17:59 message points to the edit as an example of distillation.

```diff
-A decision engine: distills a project's docs and code into the key facts, each cited to its source and confirmed by a person.
+A decision engine: distill a project's docs and code into key facts, cited by source, confirmed by humans.

-The first version distills the Markdown docs in a git repo, finding stale, redundant and conflicting claims, and links each fact to the source code it describes.
+v1 distills Markdown docs in git repos. Find stale, redundant, and conflicting claims. Link facts to source code.

-- [docs/decisions.md](docs/decisions.md): the current decisions, with citations
-- [transcripts/](transcripts/): the design sessions those decisions cite
+- [docs/decisions.md](docs/decisions.md): current decisions, cited
+- [docs/mvp-plan.md](docs/mvp-plan.md): MVP development plan
+- [transcripts/](transcripts/): prompts & transcripts
```

---

**Rachel Fish** · 2026-11-16 15:00

Hello again, my name is Rachel Fish, a principal staff software engineer. We are continuing work on this project: https://github.com/itsrachelfish/distillery

This is the fourth and hopefully final session of planning and brainstorming for this project. In previous sessions, we laid the foundation for understanding the process of distillation and came up with an implementation roadmap for an MVP which allows for the distillation of technical documentation in software & hardware projects.

In this session we will finish the remaining software architecture  decisions listed as *proposal* in the MVP planning doc.

Once we have completed this planning session, we will hand over the project to Fable for implementation. Thank you

---

**Claude**

I'll research the five open *Proposal* items (web server, browser scripts, schema versioning, API client, configuration), check your other Rust projects for conventions, then bring back options with a recommendation for each.

Your IRC log tool (`wot`) already uses axum 0.8 with maud and plain HTML forms, so I'll check what else it settled on for consistency.

I went through the five items marked *Proposal* and checked each against web-of-trust, your other Rust project with a maud web interface. M12 chose maud to stay consistent with your other projects. Two of my proposals contradict what web-of-trust already does. The plan also leaves four things open that Fable would otherwise decide alone.

## The five proposals

| # | Item | Plan says | Recommendation |
|---|---|---|---|
| 1 | Web server | axum with tokio | **Keep.** Use axum 0.8 and tokio, as in web-of-trust, where maud's `axum` feature returns pages directly. Serve on 127.0.0.1 only, because the MVP has no login. |
| 2 | Browser scripts | Plain JS for progress polling and keyboard review actions | **Change to no JavaScript**, as in web-of-trust. Review actions are HTML forms that post and redirect back. While a run is going, the run page reloads itself with `<meta http-equiv="refresh">`. The keyboard shortcuts were my idea, so I'd drop them from the MVP. |
| 3 | Schema versioning | SQLite's `user_version` | **Keep, done the way web-of-trust's `db.rs` does it.** The schema is a list of SQL steps. Each step runs in a transaction that also sets `user_version`, and the program refuses a database newer than itself. Foreign keys are on, plus WAL, a busy timeout and bundled SQLite. `rusqlite_migration` uses the user_version value the same way, but the hand-written version is about 30 lines and doesn't need the dependency. |
| 4 | Anthropic client | reqwest | **Keep.** Use reqwest 0.13 with hand-written types for only the parts of the Messages API we use. Anthropic's official client SDKs are for seven languages: Python, TypeScript, C#, Go, Java, PHP, and Ruby, and Rust isn't one of them. The unofficial crates would put their own types at the trait boundary, which I3 keeps vendor-neutral. |
| 5 | Configuration | Key from `ANTHROPIC_API_KEY`, model name in a config file | **Change to no config file**, as in web-of-trust. Settings are command-line flags with defaults: `--db`, `--port`, `--data-dir` and `--model`. I'd default to `claude-opus-5-5`, since the MVP is judged on quality and runs only cover a few files. The API key comes only from the environment, never from a flag, the database or the logs. |

**More on item 4:** every call forces one tool and turns on strict tool use, which guarantees Claude's tool inputs match your JSON Schema by constraining the model's token sampling to schema-valid outputs. Strict mode needs every object in the schema to set `additionalProperties: false`, so both schemas in the plan gain that line. Distillery's own checks can then focus on content, meaning quotes and claim IDs. Calls that fail with 429, 5xx or 529 are retried, waiting as long as `retry-after` says when the API sends it.

## Gaps the plan doesn't cover

6. **Where runs execute:** the plan only says "a background task in the server," so the choice between async tasks and a thread is still open. I recommend writing the pipeline as plain synchronous code that runs on its own thread, like web-of-trust's analysis. rusqlite, git and blocking HTTP are all synchronous, so build step 4 (command line) and step 5 (server) run the same code. Pages read progress from the database through `spawn_blocking`.
7. **Who is reviewing:** P3 records a person on every decision, but the MVP has no login. I recommend `distillery serve --reviewer "Rachel Fish"`, so every decision made through that server is recorded as that person. Each run's model is recorded as the AI party that proposed its claims (C5).
8. **Prompt version:** the cache key includes it (M8), but nothing says how it's set. I recommend keeping the prompt text and tool schemas as files in the repo, with the version being a hash of those files. Editing a prompt then clears its cache automatically, and nobody has to remember to bump a number.
9. **Choices Fable makes:** small choices will come up that no plan covers, such as an error type or a helper crate. Fable follows web-of-trust's conventions where one exists:
   - edition 2024
   - clap derive
   - a boxed error type instead of anyhow
   - Conventional Commits (R5)

   Fable lists every other new crate or design choice in its PR description as a proposal for your sign-off, so nothing becomes a decision just because it was merged.

## Left over from session 3

10. **Items without a decision number:** the plan's intro says everything without a decision number is my proposal and needs sign-off. That covers more than the five above: the pipeline checks, the data model, both extraction schemas, runs and failures, and the build order. Would you like to sign those off as a block, or go through any of them?
11. **An unanswered question:** M6 now says MVP citations open the docs. That replaces your session 2 request for citations that open the source code, which moved to L2. Session 3 ended with my request for your confirmation, and you never answered it, so please confirm.

    Separately, the session 3 transcript header says it ran to 11-14, but your last three messages are from today. I'll fix that.

Please answer each number. I'll only record what you confirm explicitly, so nothing is assumed by silence. Once everything is settled, I'll update the plan, decisions and transcript in one pass (R7) and write the handoff for Fable.

Sources:
- [Claude SDKs and libraries overview](https://platform.claude.com/docs/en/cli-sdks-libraries/overview)
- [Strict tool use](https://platform.claude.com/docs/en/agents-and-tools/tool-use/strict-tool-use)
- [rusqlite_migration](https://docs.rs/crate/rusqlite_migration/latest)
- [wetfish/web-of-trust](https://github.com/wetfish/web-of-trust): `Cargo.toml`, `src/db.rs`, `src/review/mod.rs`, `src/review/analyze.rs`

---

**Rachel Fish** · 2026-11-16 17:59

[Attached file: ai-development-guidelines.md]

@"/mnt/user-data/uploads/e54c7b4e-ai-development-guidelines.md" 1. Agreed, MVP has no login so localhost only
2. No javascript is fine, let's keep this minimal until we need it, no need for keyboard shortcuts in an MVP. Let's keep things simple
3. This is fine, let's use a similar setup to web-of-trust so we can keep things minimal without dependencies where it makes sense
4. Reqwest is approved, REST API is better than a SDK since it's more platform agnostic anyway
5. I would prefer to have a config file instead of command line arguments. For example in this project - https://github.com/wetfish/lurkers/blob/main/lurker.example.toml
6. I want to make sure we are breaking this up into separate chunks with observability as much as possible, for example in the web-of-trust repo there is real-time command line output at every stage when things are being processed. We want to have a similar thing, command line progress reports as well as a way of viewing the same progress updates through the web interface. It's really important that our UI is always clear and never appears to be frozen / blocking. We should always explain what we are doing at every step, show what is being processed, how many records exist, what the current iterator is, a progress bar or at a minimum a percentage complete at all times
7. Reviewer can be an option in the config file for now
8. Sounds good to keep the prompt tracked in git with a hash so we can tell the version and what records need to be updated
9. Decisions made by AI should always be flagged as such so that they can be reviewed and we avoid the situation where an assumption becomes a documented technical requirement without human review
10. The documentation has been updated across a few different runs. Certain decisions needed to be made now, specifically for the implementation of the MVP for parsing documentation. However things like pipeline checks, and the final data model are large projects in themselves to be iterated on later
11. Please expand on this. Do you have sufficient context from my previous answers or do you need more information? Let's confirm. Additionally, yes please update the header to fix the accuracy of the transcript and please note how I updated the readme file between commits. I noticed there were certain things that did not adhere to the writing guidelines, specifically referring to things as singular objects instead of categories. Notice the changes I made to the readme as a good example of what the distillation process should do. Even a short readme file with a few sentences could be condensed. AI writing loves adding fluff

---

**Claude**

I'll check the README history and the lurkers config example first.

I have enough context to settle almost all of item 11. One piece needs a yes or no from you, and your README edit touches the same question.

## Item 11: citations in the MVP

Session 2 had two requests about code in the MVP:

- **Existence checks:** "For an MVP I think we should just check that the specific functions exist at the cited lines" (session 2, line 291).
- **Citation targets:** "linked clickable citations that bring you to the source code" (session 2, line 295).

Your session 3 answer settles the existence checks: "focused on extracting the essence of the documentation… before we try doing anything like automatically checking the code" (line 536). Those checks moved to L2.

The citation targets aren't settled. In the MVP the only sources are Markdown docs, so a citation can open the cited doc lines at the run's commit. Linking a claim to the code it describes requires connecting docs to code, which is L2 work. My reading is that citations open doc lines in the MVP and code links come later. Your new README line "Link facts to source code" sits under "v1," so the README and M6 currently disagree.

**Needed from you:** in the MVP, do citations open only doc lines, with the README line moving to later work? Or should code links stay in v1, which brings part of L2 back into the MVP?

## What your README edit shows

| Before | After | Pattern |
|---|---|---|
| distills… the key facts, each cited to its source and confirmed by a person | distill… key facts, cited by source, confirmed by humans | Categories instead of single objects ("a person," "its source") |
| The first version distills the Markdown docs in a git repo, finding… | v1 distills Markdown docs in git repos. Find… | Short sentences, no filler articles |
| the current decisions, with citations | current decisions, cited | Noun phrases instead of prepositional padding |

The file went from 67 to 53 words without losing a fact. I'll note this in the transcript as a worked example of distillation. Items 1–9 below follow the same rules.

## Recorded from your answers

1. **Server:** axum and tokio, serving on 127.0.0.1 only. The MVP has no login.
2. **Pages:** no JavaScript and no keyboard shortcuts. Review actions are HTML forms, and run pages reload themselves.
3. **Storage:** migrations follow web-of-trust's pattern: SQL steps, `user_version`, WAL, foreign keys and bundled SQLite. No migration crate.
4. **Model API:** reqwest against the REST API, with strict tool use.
5. **Config:** `distillery.example.toml`, copied to a git-ignored `distillery.toml`, following lurkers. Proposed contents:

```toml
# Distillery's settings. Copy this file to distillery.toml.

[server]
# Port on 127.0.0.1. The MVP has no login, so Distillery only serves locally.
port = 8080

[review]
# Reviewer recorded on every decision made through this server.
reviewer = "Rachel Fish"

[model]
# Anthropic model for new runs. Each run records its model.
# The API key comes only from the ANTHROPIC_API_KEY environment variable.
name = "claude-opus-5-5"

[storage]
# SQLite database, created when missing.
database = "distillery.sqlite"
# Repository clones, one folder per repository.
repos = "repos"
```

6. **Progress:** one progress module, modeled on web-of-trust's `progress.rs`:
   - **Terminal:** one line redrawn in place, or plain lines every few seconds when output isn't a terminal.
   - **Web:** the same report on the run page, which reloads every 2 seconds.
   - **Stages:** each stage reports its label, items done out of the total, a percentage, and the current item. Each stage ends with a summary line and its duration, and the summaries are saved with the run.

   | Stage | Shown while running |
   |---|---|
   | Clone | git's own progress: objects received, percentage |
   | Files | Markdown files found, files selected |
   | Sections | Sections per file, cached and to send |
   | Atoms | Section *n* of total, file and line range, attempt number, elapsed time of the current model call |
   | Checks | Quotes verified and rejected |
   | Molecules and links | Atoms sent, molecules and links recorded |
   | Retries | Reason and countdown, such as "rate limited, retrying in 20 s" |

   A single model call has no percentage, so the page shows a running clock for the call instead. Streaming the response could also show atoms arriving one by one. I'd hold that back as a later change, to be added only if long calls still look stuck.
7. **Reviewer:** set in the config file, as above.
8. **Prompt version:** prompts and tool schemas are tracked in git, and the version is a hash of them. Records from older versions show as needing a re-run.
9. **AI decisions:** every claim, link, finding and decision records its author. Records written by AI agents carry an "AI proposal" label in every view until a person confirms them. Choices Fable makes outside the plan are marked "Proposed by Fable" in the plan and listed in its PR until you sign them off.

## To confirm

- **Item 11:** citation targets in the MVP, as asked above.
- **Item 10:** my reading is that the pipeline checks and the data model are starting points, iterated during development. Sections, the two extraction schemas, runs and failures, and the build order are signed off for the MVP. Is that right?
- **Items 5 and 6:** the config layout and the progress stages above.
- **Fable's handoff:** Fable follows your AI development guidelines. That means one commit per message with `codium` commands, full files, QA steps, and no test suites or dependency installs in the sandbox.

Once you confirm these, I'll write the plan, decisions, README and transcript in one pass. That pass includes the corrected session 3 header.

---

**Rachel Fish** · 2026-11-16 18:33

Regarding the item 11 conflict - The original idea of the project was to cite everything based on claims in source code, but we have reduced the scope for the MVP to focus on distilling markdown docs files. "v1" refers to the desired end state after we iterate, not just the first commit of the first MVP we build.

I don't think it makes sense to have repos as a config file option, this is something that is essentially run-time user data. It's user input. The repos would be added to the project via the UI, not a config file.

Finally, I want to push back on the naming convention of "atoms" and "molecules" this was intended to be an illustrative example not a specific naming request. Are there other more sensible words here for these that would make more sense in the project scope?

Confirming item 10 - What pipeline checks would we be performing? Like CI/CD pipeline in a git repo? That seems unnecessary for this initial MVP. Unless you mean like general documentation processing pipeline. Let's be clear about the language we're using

Config file structure confirmed, except for the previous note I made regarding the repos.

Let's discuss these final points and then you can draft the updated documentation and handoff

---

**Claude**

Here are the four points, each ending with what I need from you.

## Item 11: settled

I'll treat the README as describing v1, the end state after iterating, and the MVP as a subset of v1. That means the README stays as you wrote it. M6 already says MVP citations open the cited doc lines, and code citations stay in L2 on the way to v1.

The plan currently uses "v1" in places where it means the MVP, so it will say "MVP" every time it means the first build.

## Repos setting: my wording was unclear

`[storage] repos` was meant to be the folder where Distillery keeps its clones on disk. It was never meant to list repositories. I agree that repositories are user input, added through the UI and stored in the database.

The setting isn't needed at all. Clones can go in a folder next to the database, so `[storage]` keeps only `database`. That leaves one setting fewer, and nothing in the config file could be mistaken for user data.

## Names for atoms and molecules

The model only needs one record type: a claim that either quotes one span or lists other claims as its parts. The names below are labels for those two cases.

| Pair | Origin | Fit |
|---|---|---|
| **Atomic claim / compound claim** | Logic distinguishes atomic propositions from compound ones. FActScore calls its units "atomic facts." | Both terms are established, with no metaphor. "Atomic" is the standard term, not a chemistry reference. |
| Simple claim / compound claim | Logic also uses "simple proposition" for the same idea. | Avoids "atomic" entirely, but "simple" can read as "trivial." |
| Cited claim / derived claim | Requirements engineering uses "derived requirements." | Doesn't fit groupings: a table of settings is a group of claims, not a conclusion drawn from them. |
| Leaf / composite | The Composite design pattern | Describes position in the tree, not meaning. It's code vocabulary, not reviewer vocabulary. |

I recommend **atomic claim** and **compound claim**. The tools would become `record_claims` (pass 1) and `record_compounds` (pass 2). P4 keeps your quote, with a note that the atoms-and-molecules framing was an illustration.

## Item 10: "pipeline checks" meant validation, not CI

I meant the "Check" column of the plan's pipeline table. That column lists the deterministic checks Distillery runs on model output at each stage of document processing:

- Every quote appears word for word in its section.
- Every part and link points to an existing claim.
- The run's commit hash is recorded.
- Every claim ends up kept, merged or rejected.

None of this is CI/CD. CI integration is L8, after the MVP.

To keep the language clear, the plan would use these terms:

| Term | Meaning |
|---|---|
| Stage | One step of document processing: snapshot, file selection, claim extraction, compound claims and links, review, output |
| Validation | Deterministic checks on model output before anything is saved |
| MVP | The first build |
| v1 | The end state described in the README |

Three of these checks are core to the MVP. Quote validation is what keeps hallucinated claims out (C3), so M3 depends on it. ID validation and the recorded hash are both one-line checks. My revised reading of item 10:

- **Signed off for the MVP:**
  - sections
  - both extraction schemas
  - quote and ID validation
  - runs and failures
  - build order
- **Starting points, iterated during development:**
  - the data model beyond what those pieces need
  - the claim kinds
  - the link types

The glossary above would go into the plan as a short **Terms** section, so Fable and future sessions use the same words.

## To confirm

1. Clones go in a folder next to the database, and `[storage]` keeps only `database`.
2. Atomic claim and compound claim, or another pair from the table.
3. The item 10 split above, and the Terms section.

Once you've answered these, I'll draft the updated plan, decisions, README and transcript, plus the handoff for Fable.

---

**Rachel Fish** · 2026-11-16 18:54

1. It's fine to keep repos in a specific directory defined via the config, I like that idea actually, I just think it needs to be more clearly defined in this way
2. Atomic claims and compound claims sounds good
3. Yes let's keep a minimal glossary of terms so we can keep everything consistent, however I want to avoid making too much redundant documentation and I question if a separate glossary doc is actually necessary or if we can just use a small table

---

**Claude**

[Attached files: transcripts/2026-11-16-design-session-4.md, transcripts/2026-11-12-design-session-3.md, docs/decisions.md, docs/mvp-plan.md]

The final documentation is ready for review: the plan, the decisions and both transcripts. Every one of the 117 citations appears on the line it cites, the JSON schemas and config example both parse, and every link resolves.

**Open the files (same order as the attached files):**

```bash
codium transcripts/2026-11-16-design-session-4.md
codium transcripts/2026-11-12-design-session-3.md
codium docs/decisions.md
codium docs/mvp-plan.md
```

**What changed:**
- **Plan:**
  - New Terms table, Progress section and Configuration section.
  - Claims are now atomic and compound, extracted by `record_claims` and `record_compounds` with strict tool use.
  - The data model, claim kinds and link types are marked *Starting point*.
- **Clone folder:** renamed to `clone_dir`. Its comment says repositories themselves are added through the web interface.
- **Decisions:**
  - New: M9, M16, P4, R9, R10, I9 to I12.
  - Changed: M1, M6, M13, M14, P5, I6.
  - M and P are renumbered by priority (R3), with 24 new references.
- **Transcripts:** session 4 is added, including your README edit as a diff. The session 3 header now ends on 11-16.
- **README:** unchanged, since it describes v1.

**One proposal left:** runs execute as plain synchronous code on their own thread, and the command line and the server share that code. Your item 6 answer covered progress but not threading, so the plan marks it *Proposal*. Please confirm or change it before the handoff.

**QA steps:**
1. Check that both diagrams render on GitHub.
2. Read the plan for anything you wouldn't sign off, especially the Terms table and the remaining *Proposal*.
3. Spot-check references 94 to 117 against the session 4 transcript.

**Commit message:**

```text
docs: settle MVP architecture and add session 4 transcript
```

**Handoff for Fable** (paste with the AI development guidelines attached):

```text
Hello, I'm Rachel Fish, a principal staff software engineer. You're implementing the Distillery MVP: https://github.com/itsrachelfish/distillery

Read docs/mvp-plan.md and docs/decisions.md before writing code. The transcripts behind them are the source of truth (R1). Follow the attached AI development guidelines.

- Build in the plan's order, one step per commit, each leaving a working program (M5).
- Use the plan's Terms table for names in code, pages and docs.
- Use web-of-trust and lurkers (github.com/wetfish) as references for existing conventions.
- Mark choices no decision covers as "Proposed by Fable" in the plan, and list them in the PR for my sign-off (P4).

Start with build step 1: project setup.
```
