# Transcript: Distillery design session, 2026-11-11

Participants: Rachel Fish, Claude (Opus 5.5, Anthropic). Times are America/Denver (MST). Claude's messages have no recorded times. Tool calls (cloning repos, writing and discarding drafts) are omitted; message text is verbatim.

---

**Rachel Fish** · 00:58

I am a software engineer with over 25 years of experience. I've worked on dozens of AI-assisted projects now and  I've started to notice some common patterns and pitfalls. For example, paid, for-profit AI models have perverse incentives to increase the lengths of their output since users pay per token. Then, these lengthy outputs are used as inputs in future prompts. For example, when updating a feature in a project, providing the source code to that project. Now the user is paying for input tokens as the AI reads all the lines of code / documentation / automated tests.

https://github.com/itsrachelfish/distillery

This is why I've created this git repo and why I'm starting this session. I want to create a project which works with LLM services like Claude, or self-hosted instances running on dedicted hardware. I want to be able to process all of the documentation, unit tests, and source code for my projects and distill it down into facts that are supported by evidence and proven through citations.

I want this project to be written in rust and create a MVP proof of concept that uses the Claude API. I want to be able to provide a URL to a git repo and then process all of the source code, documentation, and automated tests and perform the following operations:

* Check all documentation to the source code, see what the docs say and if those things are supported in the code with citations to specific lines that actually perform the functions listed. Spot any gaps & hallucinations
* Check all source code to documented features & requirements. Flag any gaps, identify & mention code that has undefined behavior, understand what every line of code is doing
* Check all automated tests are meaningful and necessary
* Remove unnecessary information from Documentation, remove old & stale context, remove unnecessary code & unused features
* Distill a project down to the minimal amount of information that it takes to prove what the current source of truth is, with code, docs, and tests all made as concise as possible by understanding the interrelations that exist within arbitrary systems


As another example, my work has many meetings that are transcribed and saved in our documentation. And then software & simulations are made based on the decisions in those meetings. We need heuristics to track what the current state of all decisions currently is, so we have a clear picture at all times what our hardware & software is going to look like. Old code & old documentation gets stale and then conflicts with updated decisions. We have to always be tracking and confirming what the docs say versus what the code actually does throughout this distillation process.

Let's think about this a lot before we even consider writing any code.

---

**Claude**

I'll start by checking what's already in the repo, then give you my thinking.

The repo is empty so far (just a README and the MIT license), so we're starting from nothing. Since you're worried about verbosity, I'll keep this tight.

On incentives: I'm one of those paid models, so weigh my view with that in mind. Verbosity also comes from training, because human raters tend to reward longer answers, so it shows up even where nobody profits per token. Either way, your tool goes after the bigger cost: paying to re-read the same unchanged repo every session.

## 1. The core idea is a claim ledger, not a summary

Every source gets broken down into small, checkable claims: docs, comments, tests, code and meeting transcripts. Each claim carries its evidence:

- **claim**: "CLI accepts `--dry-run` and makes no network calls"
- **source**: where the claim was made (path, lines, commit SHA)
- **evidence**: code or test spans that support or refute it, each pinned to a commit SHA, line range and content hash
- **relations**: supports, contradicts, supersedes, tested-by, implements
- **status**: verified, contradicted, unsupported, uncheckable

That one structure covers both of your use cases. In each, you have competing claims of different ages and authority, and you want the current consistent set. The distilled project is then the smallest set of verified claims plus pointers to their evidence. Future AI sessions read that instead of the repo, and pull the cited source only when they need it.

## 2. The LLM is the hallucination risk inside a hallucination detector

This is the main design constraint. Three rules I'd treat as non-negotiable:

- **The model never just asserts a citation.** It has to quote the span, and Rust checks that the quote exists at those lines in the pinned commit. If the check fails, the claim is rejected, not marked "low confidence."
- **Claims that something exists and claims that it doesn't are different.** "Line 42 does X" can be checked directly. Gaps and doc hallucinations are claims of absence ("nothing implements X"), and proving absence needs a search record: what was searched, how, and what came back. Attach that record to every gap finding, because that's where the false positives will be.
- **Get a second opinion on "contradicted" and "not found" verdicts.** These are the costly ones to get wrong, so each gets a separate independent check before it's reported.

## 3. Use deterministic tools first and the LLM only where it's needed

Much of your list is better answered without a model:

- **Structure:** tree-sitter (a multi-language parser) for symbols, definitions and rough call graphs.
- **Dead code:** compiler and linter warnings, plus coverage data.
- **Meaningful tests:** mutation testing (cargo-mutants, Stryker, mutmut, PIT). These tools deliberately break the code; a test that catches none of those breaks is measurably useless, which beats any LLM opinion. The LLM's job shrinks to two questions: does this test check the *documented* behavior, and are two tests redundant?
- **Undefined behavior:** this depends on the language, and an LLM will be noisy at it. Hand it to sanitizers, Miri, clippy or static analyzers, and let the LLM explain their findings rather than invent its own.
- **Staleness:** git history is a cheap, strong signal. A doc untouched since 2023 that describes a module with 40 commits since then is suspect before any model reads it.

## 4. Fetch what's needed instead of stuffing the context

Rather than feeding in the whole repo, give the model tools (search symbols, grep, read a function, list callers) and a token budget per claim, and let it fetch only what it needs. That fits your cost goal and works on repos bigger than any context window. Cache every summary and verdict by content hash, so a rerun only pays for what changed. Incremental runs on diffs are where the savings really add up.

## 5. Who's right when docs and code disagree?

The tool can't know. Sometimes the doc is the requirement and the code is the bug; sometimes the doc is just stale. So conflicts should be shown with evidence for both sides and a timeline (what changed last, from git), not resolved automatically.

For meetings, each decision needs a date, a topic, who made it, and explicit links to the decisions it replaces. The current state is then the decision log replayed in order, much like architecture decision records (ADRs). The hard part is knowing two decisions are about the same thing: that Tuesday's "drop the second sensor" replaces March's "dual-sensor design." I'd have the LLM propose those links and a human confirm them, because a wrong link silently erases a decision that's still live.

## 6. Pruning should propose changes, never apply them

"Remove unused code and docs" is the riskiest operation. Code that looks unused may still be reached through reflection, config, feature flags, FFI, or outside users of a public API. Deliver deletions as a patch or PR in which every change states its evidence: no references found (with the search record), zero coverage, no effect under mutation testing, or doc replaced by a newer decision. Add confidence tiers, and leave the merge to a human.

## 7. Build a benchmark early, or we won't know it works

Make a small set of test repos with planted faults:

- a documented flag that doesn't exist
- a dead function
- a test that can't fail
- a stale doc section
- a decision that was later replaced

Then measure, for each fault type, how many real problems it finds and how many of its findings are wrong. We can also run it on distillery itself.

## MVP scope

Your five operations form a pipeline, and the last two (pruning and distilling) depend on the first three. I'd make the MVP **operation 1 only: checking docs against code, with verified citations.** It exercises every hard piece and is the easiest to measure:

1. Clone the repo, pin the commit SHA, and sort files into docs, code, tests and config.
2. Index with tree-sitter plus plain text search.
3. Break the docs into small, checkable claims, each with a verified quote from the doc. Set aside claims that can't be checked (marketing fluff); they become free input for pruning later.
4. For each claim, let the model search the code within a budget, then return a verdict with quoted code citations.
5. Rust verifies every quote, then writes a JSON ledger and a markdown report.

In Rust, that means an `LlmProvider` trait with two backends:

- **Claude**, using tool use to get structured output.
- **OpenAI-compatible**, which covers vLLM, llama.cpp and Ollama.

Validate every response against a schema and retry on failure, since smaller self-hosted models are less reliable at structured output. Build cost controls in from the start: a token estimate before each run, a hard budget cap, prompt caching for the system prompt and tool definitions that repeat on every call, and batch mode for runs nobody is waiting on.

## Questions that change the design

1. **Which languages are your target repos in?** That decides which tree-sitter grammars and analyzers we need, and how much the undefined-behavior check matters.
2. **At work, is there a rule for whether docs or code wins a conflict, or does it depend?** And are meeting decisions recorded in any structured form today, or only as raw transcripts?
3. **What models and hardware for self-hosting?** Mid-sized open models mean smaller contexts and shakier tool use, which pushes more of the work into the deterministic layer.
4. **What's the end output: a report, a context pack for future AI sessions, or PRs against the repo?** My guess is a report for the MVP, with the context pack as the real product.

Once we've settled those, I'd write this up as a `DESIGN.md` in the repo, including the ledger schema, before writing any Rust.

---

**Rachel Fish** · 01:19

Great idea on the 1 operation MVP (checking docs against code, with verified citations). We need to be as language agnostic as possible, I want to support projects written at a minimum: PHP, Rust, Python, JavaScript (& Node.js), Go. We can start with PHP as a starting language to process, but I want this project itself to be written in Rust.

1. PHP, Rust, Python, JavaScript, Go
2. Meeting transcripts, ordered by recency, specifically who said what. Who are the responsible engineers and what have they said in meetings about what is being built? Then, compare that against all documentation and identify gaps & contradictions. Then, compare all source code to the approved documentation & requirements and identify gaps in the code & things that need to be updated
3. I'm not sure, I have to experiment more, for now I want to develop this in an architecture where we can use other platforms, but for now just using the Claude API as a benchmark.
4. Ideal end state: PRs against the repo, issues created, integration with Linear. Initial MVP: a report of changes that could be made in PRs and a report of issues that could be created.


One thing that hasn't been discussed here yet is the fact that most of this will require some level of review & approval. There needs to be an interface where people can actually review the citations and agree or disagree with the decisions and record why. This project is about taking other projects and making them better by distilling them down to their essence.

I agree that we should be building this in such a way that we actually avoid using LLMs and use traditional code analysis tools as much as possible

Basically, what we're building: a way for AI to process a project and then display all of the facts that have been decided by the code, the requirements, the documentation, etc. The user says what the source of truth is. When we start analyzing a new project we always need to know: what is the source of truth? Then we start analyzing that. The documentation, transcripts, or source code, and building out a tree of relationships and a way to view the relationships and confirm them.

We are building an information relationship visualization interface

---

**Claude**

That framing clarifies a lot. The product is the reviewed graph of relationships; the static tools and the LLM are just ways of proposing links for people to confirm. Here's what changes.

## Source of truth is a ranking, not a single source

The workflow you described is a chain, where each layer is checked against the one above it:

1. Decisions made by responsible engineers in meetings, newest first
2. Approved docs and requirements
3. Code and tests

So the project config shouldn't name one source of truth. It should declare this ranking, and allow different rankings for different areas; for a legacy module, the code may be the truth and the docs should follow it.

The word "approved" also matters: a doc only becomes authoritative once a person approves it. That puts human review decisions at the top of the chain. Over time, the confirmed graph becomes the source of truth, rather than any single input. That's the lasting thing the tool produces.

## The graph data model

- **Nodes:** sources (a range of lines in a file at a specific commit, or one utterance in a transcript), people, and claims.
- **Edges:** typed relationships such as implements, contradicts, supersedes, said-by, owns, tested-by.
- **How each edge was found:** the tool that produced it (a tree-sitter query, PHPStan, git blame), or the model, prompt version and run. You can see at a glance whether a link is deterministic or an LLM guess. Reviews then tell you how accurate each method is.
- **Review state on every edge:** proposed, confirmed, rejected or needs change. Rejecting requires a reason, and the reviewer and time are recorded.

One hard problem to design for now: reviews have to survive code changes. When a confirmed citation's lines move or change, the tool finds the text again by its content hash, much like PR comments survive a rebase. Unchanged text keeps its review. Changed text goes back to the queue as "needs re-review," with a diff. Otherwise every commit wipes out the review work.

Rejections with reasons are also feedback. They become examples and suppression rules, so the same false positive doesn't come back on the next run.

## Meetings: a statement isn't a decision

"Who said what" is the easy part, as long as transcripts have speaker labels. The harder parts:

- **Classifying statements:** each relevant utterance is a proposal, opinion, question, decision or action item. Only decisions count toward the truth. The LLM proposes a label and a person confirms it.
- **Knowing who has authority:** a declared list of who owns which area beats guessing. CODEOWNERS and git blame tie people to code without any LLM, which gives a free cross-check: "Sam decided X about billing, Sam owns /billing, and /billing doesn't do X."
- **Bad speaker labels:** transcripts often attribute lines to the wrong person, so reviewers need a way to fix that.
- **Supersession:** a later decision on the same topic, by someone with authority, replaces earlier ones. Deciding that two decisions are about the same topic is fuzzy, so these links are always proposed for review, never applied automatically.

## The review interface

Graph views look great with 50 nodes and turn into a hairball at 5,000, so I'd split the interface in two:

- **Review queue, where the work happens:** proposed findings, sorted by severity with contradictions first. Each opens a detail view showing:
  - the claim
  - the cited code, with the relevant lines highlighted
  - the doc or transcript excerpt, with speaker and timestamp
  - buttons to agree, disagree or mark "needs change," with a reason
- **Graph view, for exploring:** one node and everything within one or two hops of it ("show me everything connected to this requirement"), rather than the whole project at once.

Reports, and later PRs, issues and Linear, only ever use confirmed findings. Nothing leaves the tool without a person approving it.

## Language support

All five languages have mature tree-sitter grammars. Each language gets an adapter, and most of an adapter is a set of tree-sitter query files rather than Rust code. The queries find definitions, references, entry points (routes, CLI commands) and tests. Adding a language mostly means writing queries. Later, each language plugs in its own analyzers:

| Language | Static analysis | Tests | Mutation testing |
|---|---|---|---|
| PHP | PHPStan / Psalm | PHPUnit / Pest | Infection |
| Rust | clippy | cargo test | cargo-mutants |
| Python | mypy / ruff | pytest | mutmut |
| JS / TS | tsc / ESLint | Jest / Vitest | Stryker |
| Go | go vet / staticcheck | go test | Gremlins |

PHP is a good language to start with because it's the hardest of the five to analyze statically. It has magic methods (`__call`, `__get`), functions called by string name, and framework routes defined in config files or attributes. That makes "not found in the code" a weaker finding in PHP than in Go or Rust. So the tool needs framework adapters (for example, one that reads Laravel's route files), and every "not found" finding should state how confident it is.

## Architecture

One Rust binary with three commands:

- **`distillery analyze <repo-url>`:** clones and indexes the repo, then writes proposed findings into a SQLite file. The graph is stored as node and edge tables, which needs no server and is plenty for the MVP.
- **`distillery serve`:** runs a local web server (axum) with the review interface.
- **`distillery report`:** writes the proposed PRs and issues as markdown and JSON, using confirmed findings only.

LLM access sits behind a provider trait, with Claude as the benchmark backend. The planted-fault benchmark repos from before can then compare backends on the same inputs.

## Revised MVP

The MVP is docs checked against code for PHP, plus a minimal review interface (queue, detail view, one-node graph view) and the report.

Transcripts come second. They're the top of your chain, but they bring their own problems: transcript formats, speaker identity and classifying decisions. Docs-against-code exercises the whole pipeline first, and meetings then plug in as one more layer of the same graph.

## Questions

1. **Transcripts:** what tool produces them (Zoom, Teams, Meet, Otter), and in what format? Is there a list of who owns what?
2. **PHP frameworks:** Laravel, Symfony, WordPress or plain PHP? That decides which framework adapter to build first.
3. **Reviewers:** a single local user for the MVP, or multiple reviewers with logins and approval permissions from the start?
4. **Frontend:** TypeScript, which has the best graph libraries (such as Cytoscape.js), or Rust compiled to WebAssembly, which keeps one language but has a thinner set of graph libraries? I'd lean toward TypeScript for the UI only.

Once those are answered, I'll draft `DESIGN.md` with the graph schema, the format for the source-of-truth config, and the review states.

---

**Rachel Fish** · 01:26

> One hard problem to design for now: reviews have to survive code changes. When a confirmed citation’s lines move or change, the tool finds the text again by its content hash, much like PR comments survive a rebase. Unchanged text keeps its review. Changed text goes back to the queue as “needs re-review,” with a diff. Otherwise every commit wipes out the review work.

I disagree that this is a hard problem. Instead, we start with a static analysis tool which allows us to run this report at a single snapshot in time. Then we can deploy this tool using a CI/CD build system and compare every new commit to an existing repo against this confirmed source of truth graph database.

We can automatically flag things like where someone submits a PR with code that conflicts against the confirmed requirements for the project, and only allow new code to be merged when it has cited documentation. That's the long term direction of the project. f

But for right now, a single analysis run with a report.

---

**Claude**

You're right, and I overstated it. If every change goes through CI, you never have to reconcile a large drift after the fact. Each PR is a small diff, and git's own diff tells you exactly which lines moved or changed. Any confirmed citation that a PR touches gets flagged in that PR's check, and gets re-reviewed alongside the code that changed it. That's simpler and more reliable than finding text again by content hash. It also turns the graph into a merge gate, which is a strong long-term direction.

The cases that still slip past the gate are narrow and can wait: direct pushes or rewritten history that skip CI, and sources that live outside the repo (docs in a wiki, transcripts) changing between runs.

One cheap thing to do now so CI mode works later: pin every citation in the snapshot report to the commit SHA, file path, line range and content hash. It costs nothing in a single run, and it's what a later PR check needs in order to compare new commits against the confirmed graph.

So the MVP is:

1. `distillery analyze <repo-url>` on one PHP repo, at one commit
2. Pull checkable claims from the docs
3. Find supporting code with tree-sitter and search first, using the LLM only where needed
4. Mechanically verify every citation
5. Write a report of proposed PR changes and proposed issues

That leaves one scoping question: is the review interface part of this first milestone, or does it come after the report? The report alone is a much smaller first build. If the report uses the same claim and citation format the interface will later read, nothing gets thrown away.

The earlier questions on PHP frameworks and frontend language only matter once we get to those pieces, so they can wait.

---

**Rachel Fish** · 01:31

The review interface and building out the tree of decisions and source of truth are the most important pieces we need to be building in this project, and therefore the MVP has to cover these pieces. Otherwise, the output report and PR code suggestions would be meaningless. Every decision should be confirmed, and it should say by who. On a single solo project this might just be one person. But an entire team of people could be using this software to analyze documentation & code and each person could be reviewing and approving decisions for specific departments, sections of the project, specific repos, etc.

This project could be used by individuals or entire teams of people deciding their core business decisions

---

**Claude**

Agreed. Then the review layer is the product, and the analysis pipeline is just what feeds it. That moves most of the MVP effort into the decision graph and the interface, and means multiple users have to be in the data model from day one, even if the first user is you alone.

## Who approved it is part of every decision

- **Every person has an identity, including a solo user.** Every review records who, when, the verdict and the reason.
- **Reviews are an append-only log.** Nothing is edited or deleted, and the current state of each decision is computed from the log. If teams use this for core business decisions, being able to trace why something is true and who said so is the whole point.
- **Confirmed means confirmed by someone with authority over that area.** Anyone can comment, but only people who own that scope can confirm.

## Scopes: who can approve what

Scopes form a hierarchy: organization, then repo, then path or doc section, plus cross-cutting tags for departments or topics (billing, firmware, security). Each scope has:

- **owners**, who can confirm or reject findings in it
- **an approval rule**: one owner, N owners, or a specific role

Code and doc scopes can be matched by path patterns, in the same style as CODEOWNERS, which can be imported directly. Topic and department scopes are tags on claims and decisions.

For the MVP, I'd support exactly one approval rule (any one owner of the scope), but keep the schema ready for the others.

## When reviewers disagree

If two owners of the same scope reach different verdicts, the tool shouldn't let the last one win. The disagreement becomes its own item in the queue and goes up to the owner of the parent scope. The same applies when a decision in one scope contradicts a confirmed decision in another, such as firmware versus billing. Those cross-department conflicts are some of the most valuable things the tool can surface.

## The decision graph

Decisions are first-class nodes, not just labels on edges. A decision has:

- a statement
- a scope
- a status (proposed, confirmed, rejected, superseded)
- the evidence it rests on (citations)
- links to the decisions it supersedes
- its review history

Claims taken from docs and code are checked against confirmed decisions. That's how "the user says what the source of truth is" works in practice: the source-of-truth ranking decides which sources can propose decisions, and people turn proposals into confirmed decisions.

## Revised MVP

To keep this buildable, I'd cut scope everywhere except the review layer:

- **In:**
  - one PHP repo at one commit
  - claims checked from docs against code
  - verified citations
  - a decision graph with identities, scopes and an append-only review log
  - a review queue, a detail view and a one-node graph view
  - a report built only from confirmed findings
- **Out for now:**
  - transcripts
  - other languages
  - CI mode
  - opening PRs, issues or Linear tickets
  - approval rules beyond "one owner"

On storage: the tool becomes a shared server for a team rather than a local tool. SQLite still handles team-scale review traffic fine. If the database code stays behind a small interface (sqlx supports both SQLite and Postgres), moving to Postgres later is easy.

## Questions that now matter

1. **Login:** GitHub login (identities match commit authors and CODEOWNERS automatically), your company's single sign-on (anything that supports OpenID Connect), or simple local accounts for the MVP? I'd lean toward GitHub login first, since the analysis already ties people to code through git.
2. **Frontend:** since the interface is now the core of the product, TypeScript (best graph libraries), or Rust compiled to WebAssembly (one language end to end)? I'd still lean toward TypeScript for the UI.
3. **First PHP project:** is there a specific repo you'd use as the first real test case? Knowing its framework and doc style would shape the first adapter and the benchmark.

Once you've answered those, I'll write `DESIGN.md` for the repo, covering the graph schema, scopes and approval rules, the review log, and the MVP scope above.

---

**Rachel Fish** · 01:34

[Attached file: ai-development-guidelines.md]

@"/mnt/user-data/uploads/e7b7b3e6-ai-development-guidelines.md" Now that we have thought about this a little bit, I would like to start making our own documentation within the Distillery repo capturing the decisions we are discussing here.

Additionally, I would like to provide for you a repo that is used to generate PHP (Laravel) projects which can be used as a general purpose implementation of the type of project we should be building for https://github.com/wetfish/skeleton

---

**Claude**

I'll read the skeleton repo first so the Distillery docs follow its conventions and so I know what kind of project Distillery will be analyzing.

The skeleton puts numbered docs under `docs/`, so I'll use the same layout for Distillery. That includes keeping `05-ai-development-notes.md` and `06-planned-features.md` in the same slots.

[Request interrupted by user]

---

**Rachel Fish** · 01:40

Sorry, I had to stop you. I don't want to copy the exact structure of the skeleton repo, I just provided it as an early example. This is a great example of the type of behavior that I want to capture.

One assumption can balloon into an entire implementation that diverges from the actual requirements: of distilling a project down to its essence. Copying the structure of this single repo will not help you analyze the structure of future repos

We have to be distilling things down to their most basic essence and proving every fact, building up a network of decisions that creates a spec of what is supposed to be

---

**Claude**

You're right. I took one example repo and was about to build a structure around it: numbered doc slots, and a table mapping skeleton doc files to Laravel paths. Those were assumptions, not requirements. I'm throwing those drafts away (nothing was sent or committed).

That behavior is also a requirement for Distillery itself: it has to work out each project's structure from evidence in that project, and never carry over assumptions from another one.

Applying that to our own docs: each decision in the repo should state who confirmed it and cite its evidence, and nothing should go in that you haven't confirmed. For now the only evidence is what you've said in this conversation. Below is every decision I can trace to your own words. My proposals (the claim ledger, quote verification, scopes and so on) are left out until you confirm them one at a time.

**Candidate decisions, each with your words as evidence:**

1. **What it is:** an information relationship visualization interface. ("We are building an information relationship visualization interface")
2. **Goal:** distill projects to their essence, prove every fact, and build a network of decisions that forms the spec. ("building up a network of decisions that creates a spec of what is supposed to be")
3. **No borrowed structure:** project structure is discovered, never assumed from another project. ("Copying the structure of this single repo will not help you analyze the structure of future repos")
4. **Rust:** Distillery itself is written in Rust. ("I want this project itself to be written in Rust")
5. **Languages:** as language-agnostic as possible; at minimum PHP, Rust, Python, JavaScript/Node.js and Go. ("We need to be as language agnostic as possible")
6. **PHP first:** PHP is the first language to process. ("We can start with PHP as a starting language to process")
7. **LLM platforms:** the architecture supports multiple LLM platforms, with the Claude API as the benchmark. ("for now just using the Claude API as a benchmark")
8. **Traditional tools first:** use traditional code analysis tools over LLMs wherever possible. ("use traditional code analysis tools as much as possible")
9. **Source of truth:** every analysis starts with the user declaring the source of truth. ("we always need to know: what is the source of truth?")
10. **Confirmation:** every decision is confirmed by a person and records who. ("Every decision should be confirmed, and it should say by who.")
11. **Scoped reviewers:** reviewers approve decisions for specific departments, sections or repos. ("each person could be reviewing and approving decisions for specific departments, sections of the project, specific repos")
12. **Recording why:** reviewers agree or disagree and record why. ("agree or disagree with the decisions and record why")
13. **MVP essentials:** the MVP includes the review interface and the decision graph. ("the MVP has to cover these pieces")
14. **MVP operation:** check docs against code, with verified citations. ("Great idea on the 1 operation MVP")
15. **MVP run:** a single analysis of one snapshot. ("for right now, a single analysis run with a report")
16. **MVP output:** a report of changes that could be made as PRs and issues that could be created. ("Initial MVP: a report of changes that could be made in PRs and a report of issues that could be created")
17. **CI mode (later):** run in CI, flag PRs that conflict with confirmed requirements, and only allow merging code that has cited documentation. ("only allow new code to be merged when it has cited documentation")
18. **Integrations (later):** PRs, issues and Linear integration. ("Ideal end state: PRs against the repo, issues created, integration with Linear")
19. **Meetings (later):** order transcripts by recency and track who said what and who the responsible engineers are; compare those decisions to the docs, then compare the code to the approved docs and requirements. ("Meeting transcripts, ordered by recency, specifically who said what")

**Questions before I write anything:**

- **Corrections:** should any of these be struck or reworded?
- **Format:** I'd suggest a README that states the essence, plus one decisions file where each entry has an ID, the statement, who confirmed it, the date, the evidence quote, and links to related decisions. Nothing else until it's needed. Does that work?
- **Guidelines:** should your AI development guidelines go in the repo as their own file, or stay as instructions for me only?

---

**Rachel Fish** · 01:53

The candidate decisions you've provided seem good, but the citations as evidence are redundant in this format and mostly simply restate what you said. In my opinion it would be better to provide htis conversation as a transcript in the distillery project and then mention citations referencing the transcript. ([1], [2], [3], etc.) with a reference of line numbers and quotes at the bottom. In this way, a transcript, a docs file, or lines of source code can all be used interchangeably

---

**Claude**

I'll add the conversation as a transcript file, then write a decisions file that cites it by line number. After that I'll run a check that every quote really appears on the line it cites.

Now I'll find the line for each quote. The search is limited to your messages, so citations point at what you said rather than where I repeated it.

Each quote was found exactly once, and only in your messages. Now the decisions file:

Now the check: parse every reference, confirm its quote appears on the line it cites, confirm every [n] used in the table has a reference, and confirm the links resolve.

[Attached files: transcripts/2026-11-11-design-session.md, docs/decisions.md]

I've written two files: the conversation as a transcript, and a decisions file that cites it. All 22 citations passed a check that each quote appears exactly on the line it cites.

**Open the files (same order as the attached files):**

```bash
codium transcripts/2026-11-11-design-session.md
codium docs/decisions.md
```

- **What's in the transcript:** every message, word for word up to your 01:53 message, typos included. Tool calls are left out. I also added two notes, `[Attached file: ...]` and `[Request interrupted by user]`.
- **What's in the decisions file:** a table of D1–D20 with numbered citations. The references at the bottom give the file, line and exact quote. Citations only point at your own messages, never at my restatements of them. D20 is the citation format you just described.
- **Attribution:** I credited you as "Rachel Fish" because that's the name in `LICENSE`. Tell me if it should be something else.
- **The check:** a short script parses each reference, confirms the quote is on that line, and confirms every `[n]` in the table has a reference. It's a small preview of the citation check Distillery itself will need. I left it out of the repo because you haven't decided on it yet.

**QA steps:**

1. Read the D1–D20 wording in `docs/decisions.md` and strike or reword anything that's off.
2. Spot-check a few references: open the transcript at the cited line and confirm the quote is there.
3. Click the links in the References section and confirm they open the transcript.

Editing the transcript will shift line numbers and break citations. That's expected, and it's the kind of drift Distillery should flag.

**Commit message:**

```text
Add design session transcript and decision log citing it
```

Three choices I left for you:

- The README is unchanged and doesn't link to either file yet.
- The `transcripts/` and `docs/` folder names were my pick.
- Whether your AI development guidelines go in the repo is still undecided.

---

**Rachel Fish** · 02:04

Please update your commit message, the transcript, and the decisions list to include the use of Conventional Commits, https://www.conventionalcommits.org/en/v1.0.0/

I want to ensure that we are following these best practices as we build this project and as we create PRs & issues for other projects. Thank you
