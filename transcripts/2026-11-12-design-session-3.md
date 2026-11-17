# Transcript: Distillery design session 3, 2026-11-12 to 2026-11-16

Participants: Rachel Fish, Claude (Opus 5.5, Anthropic). Times are America/Denver (MST). Continues [session 2](2026-11-11-design-session-2.md). Tool calls are omitted. Diagrams are replaced by a note naming the diagram shown; [docs/mvp-plan.md](../docs/mvp-plan.md) has the pipeline and data model diagrams. Message text is verbatim, except that citation markup is removed from Claude's messages and blank lines separate Rachel's one-line paragraphs.

---

**Rachel Fish** · 2026-11-12 23:44

[Attached file: ai-development-guidelines.md]

@"/mnt/user-data/uploads/6679d233-ai-development-guidelines.md" Hello again, my name is Rachel Fish, a senior principal staff software engineer. We are continuing work on this project: https://github.com/itsrachelfish/distillery

This is the third session of planning and brainstorming and it feels like we are getting close. In previous sessions, we laid the foundation for understanding the process of distillation and came up with an implementation roadmap for an MVP which allows for the distillation of technical documentation in software & hardware projects.

In this session we will perform more technical software application architecture work. In another project I've worked on previously, I built a Claude API implementation for processing information from resumes and generating new ones. https://github.com/wetfish/success

I felt that these previous Claude API implementations were limited since it only allowed for a single prompt and response, there was no agentic workflow at that time. Has this changed or is it still the case? Is this one potential reason why we need to host an agent on our own hardware?

I've also attached some AI development guidelines for your reference, Let me know if you have any questions.

---

**Claude**

I'll check our earlier sessions and both repos, then verify the current Claude API capabilities before answering.

Yes, this has changed. It also wasn't quite the limit you hit in Success. Agentic workflows on their own aren't a reason to host on your own hardware, though a few other reasons still hold up.

## What Success actually does

I read the code. `ClaudeExtractionProvider` and `ResumeAiService` both POST to `/v1/messages` with one user message and no `tools` array. They then parse JSON out of the reply text (`parseDrafts`). The resume flow is a pipeline your PHP code runs as separate one-shot calls: `analyzeRelevance`, then `generateDraft`.

The Messages API is still a single request and a single response, and it's stateless. The "agentic" part has always been a loop around it. Tool use was already available when Success was written; the code just didn't use it. Anthropic's tool-use docs make a point that fits Success directly: if you're parsing a decision out of model output, that decision should have been a tool call.

## The options now

| Option | Who runs the loop | Where tools run | Fit for Distillery |
|---|---|---|---|
| Messages API with your own tools | Your server (a `while stop_reason == "tool_use"` loop) | Your server | Best fit: Rust, full control, any provider |
| Server tools (web search, web fetch, code execution) | Anthropic, inside one request | Anthropic | Not needed for the MVP |
| Agent SDK | Your machine, using the Claude Code harness | Your machine | The same tools, agent loop, and context management that power Claude Code, but only two packages, one per language (Python and TypeScript), so not Rust |
| Managed Agents (beta) | Anthropic's hosted harness | Anthropic's cloud sandbox, or a self-hosted sandbox | Hosted REST API, a separate product from the Agent SDK. Not eligible for zero data retention unless fully self-hosted |

## Does this mean we need our own hardware?

Not for the agent loop. There are two separate questions here:

- **Where the loop runs:** the Distillery Rust server (M2) already is "our own hardware." It clones the repo, gives the model tools, runs each tool call, and sends the results back. None of that needs a self-hosted model.
- **Where the model runs:** Claude API or a local model, behind the provider trait from I2.

These reasons for owning the hardware still hold:

- **Your original goal:** in the first session you asked for support for "self-hosted instances running on dedicated hardware."
- **Data sensitivity:** private repos and meeting transcripts sent to a hosted API, and Managed Agents keeping session history on Anthropic's servers.
- **Deterministic tooling:** git, tree-sitter, and later PHPStan and test suites need a machine with the repo checked out. That's your server either way.
- **Cost at scale:** this applies to repeated full-repo runs.

## What I'd propose (not a decision yet)

Distillery owns its agent loop in Rust over the Messages API. The tools are Distillery's own deterministic functions: read a file range, search the repo, look up a symbol, check that a cited function exists at the cited line (M5). Findings are reported through a strict-schema tool instead of parsed from text. This fits I3 and the trust rule from the DeepMind discussion. The model can only touch the repo through tools Distillery controls, and only the checker decides whether a citation is valid. Self-hosted models plug in behind the same trait, though smaller open models are less reliable at tool use, so the deterministic layer carries more of the work for them.

Two questions before I write any of this into `docs/decisions.md`:

1. **Data constraints:** can the repos and transcripts you'll analyze go to a hosted API at all, or does some of them have to stay on your hardware? That decides whether a self-hosted model is an MVP need or a later option.
2. **Loop ownership:** are you happy with Distillery running its own loop in Rust, rather than using the Agent SDK as a Python or TypeScript sidecar?

Sources:
- [How tool use works](https://platform.claude.com/docs/en/agents-and-tools/tool-use/how-tool-use-works)
- [Claude Managed Agents overview](https://platform.claude.com/docs/en/managed-agents/overview)
- [Agent SDK overview](https://code.claude.com/docs/en/agent-sdk/overview)
- [wetfish/success](https://github.com/wetfish/success): `laravel/app/Services/Extraction/ClaudeExtractionProvider.php`, `laravel/app/Services/Resume/ResumeAiService.php`

---

**Rachel Fish** · 2026-11-13 00:06

We can use the claude API for now for the MVP but we have to design this in such a way that we can do our own on-premises hosting sooner rather than later. That opens the project up to run on a much broader set of problems.

Tell me more about the Agent SDK. Is that a claude API feature? If so, my worry with that would be vendor lock-in. I think rust running the agent loop makes the most sense, as it allows the model running the loop to be swapped out easily. Different projects might want to use claude, chatgpt, or other models.

---

**Claude**

I'll check the Agent SDK docs for how it's licensed and which models it supports.

Yes, the Agent SDK would lock Distillery to Claude, so I agree with running the loop in Rust.

## What the Agent SDK is

It isn't a Claude API feature. It's a client library that runs on your machine and calls the Claude API. The Agent SDK gives you the same tools, agent loop, and context management that power Claude Code, programmable in Python and TypeScript. The docs call it "a library that runs the Claude Code binary." It comes with Claude Code's built-in file and shell tools, permissions, sessions, hooks, subagents and MCP support.

It locks you in at three levels:

- **Model:** it only runs Claude. Authentication is a Console API key or Amazon Bedrock, Google Cloud's Agent Platform or Microsoft Foundry credentials. Those are different ways to pay for Claude, not other models.
- **Runtime:** it's Python or TypeScript wrapped around a binary Anthropic ships. In terms Anthropic publishes for running Claude Code inside products, the Claude Code binary must not be modified.
- **Terms:** use of the Claude Agent SDK is governed by Anthropic's Commercial Terms of Service, including when you use it to power products and services that you make available to your own customers and end users, except to the extent a specific component or dependency is covered by a different license.

It's a good fit for a product built only on Claude. For Distillery it would put a second runtime next to the Rust server, and you'd have to rewrite that part to support another model.

## Designing for swappable models

The main risk is ending up with a loop shaped around Claude. Distillery should own everything that defines its behavior, and each provider adapter should only translate:

- **Owned by Distillery:** conversation history, tool definitions (JSON Schema, which every major provider accepts), the loop itself, budget caps, retries, schema validation of tool arguments, and the citation checker.
- **Owned by each adapter:** converting to and from the provider's wire format, mapping stop reasons onto one shared set, and reporting token usage.

Here's a rough Rust sketch of that boundary:

```rust
#[async_trait]
trait ModelProvider {
    /// Context window, parallel tool calls, strict schemas, and similar limits
    fn capabilities(&self) -> Capabilities;
    /// One model turn: internal messages and tools in, text and tool calls out
    async fn complete(&self, turn: &Turn) -> Result<Reply>;
}

enum StopReason { ToolCalls, Done, MaxTokens, Refused }
```

To get to on-premises hosting sooner rather than later, I'd also propose:

- **Two adapters from the start:** the Anthropic Messages API, plus an OpenAI-style chat completions adapter. OpenAI uses that format, and so do most self-hosted model servers. Building both early stops the trait from quietly taking on Claude's shape. A small local model is enough to test the second adapter.
- **No reliance on hosted-only features:** Anthropic's server-run tools (web search, code execution) and Managed Agents stay out of the core loop. Prompt caching can be an optimization that adapters use when the provider supports it, never something the loop depends on.
- **Per-provider capabilities:** smaller self-hosted models have shorter context windows and less reliable tool calls. The loop reads `capabilities()` and adjusts by sending smaller file chunks, fewer tools per turn and stricter validation, instead of assuming Claude's limits.
- **The benchmark compares providers:** the reversed bug-fix and planted-fault repos run on every adapter, so swapping models is a measured decision.
- **Endpoint as configuration:** base URL, model name and key per project. Then an air-gapped install only needs the model server on the local network.

## Recording this

These refine I2 in `docs/decisions.md`: Claude API for the MVP, on-premises hosting early on, the Rust server running the agent loop, and models swappable per project (Claude, ChatGPT or others). The two-adapter plan and the rest of the list above are my proposals until you confirm them.

Should I add your decisions and this session's transcript as one commit now, or wait until the end of the session so the transcript is complete?

Sources:
- [Agent SDK overview](https://code.claude.com/docs/en/agent-sdk/overview)
- [Claude Code legal and compliance](https://code.claude.com/docs/en/legal-and-compliance)
- [How tool use works](https://platform.claude.com/docs/en/agents-and-tools/tool-use/how-tool-use-works)

---

**Rachel Fish** · 2026-11-13 00:14

Let's continue building up context before we update the docs. I think it's better to update the docs once at the end with our final agreed upon state instead of incrementally rewriting it multiple times. Think about how much wasted effort is spent writing docs files every commit in a PR with 20 different commits. It would be more efficient to only write the docs at the end once the agreed upon state from the requirements had been reached

Like I said before, we should be thinking about the full program architecture of what we're building now. How do we actually distill docs? What, mechanically, are we doing? We are git cloning a repo, looking for md files, and then processing them with the claude API using a specific prompt? Previously we had discussed processing all of the source code and generating our own idea of what we think the docs should be, and then diffing that against the actual docs if any exist to look for discrepancies or undocumented features

We need to be thinking about the interface we are building, what is the user experience going to be? How do we visualize meaningful information about arbitrary programs?

---

**Claude**

I'll read the rest of the decision log first so the architecture builds on what we've already agreed.

That makes sense: we'll keep building context now and write the docs once at the end.

Mechanically, "send the markdown to Claude with a prompt" doesn't work for what we want. Nothing in the output can be audited, long docs won't fit, and the model can reword or invent facts without anyone catching it. Every session so far, we've been running the real process by hand. In the first session, I pulled your statements out as exact quotes. A script then checked that each quote appeared exactly once in the transcript, and the decisions file cites only verified quotes. That's the pipeline, and `docs/decisions.md` is roughly the output format.

## The unit of work: a claim

Everything runs on claims. A claim is one short statement tied to the exact span of text or code it came from. Docs, code and transcripts all reduce to claims, which is how R4 ("cited the same way") holds mechanically.

Here's the pipeline, with LLM steps in purple and deterministic steps in teal.

[Diagram: Distillery MVP pipeline]

## What each stage does

**1. Snapshot.** Clone at one commit (M8) and list the tracked files. Each file's git blob hash becomes the cache key, so a re-run only reprocesses changed content. That also lays the groundwork for CI mode (L4) later.

**2a. Doc claims (LLM, with a deterministic check).**
- Parse each Markdown file into sections with line ranges, plus links, inline code (`ResumeAiService`) and file paths. No LLM is needed for this part.
- For each section, the model calls a `record_claim` tool with the claim text, a kind (behavior, configuration, requirement, decision, how-to, rationale), the exact quote it rests on, and its subjects.
- The check: a claim is rejected unless its quote appears in that section word for word. This is the step that keeps hallucinations out, and it works the same on weaker local models, because a bad claim gets rejected instead of trusted.

**2b. Code facts (deterministic).**
- A tree-sitter symbol index: definitions and references, with file and line.
- Entry points from framework adapters: routes, CLI commands, config keys, environment variables, migrations.
- Git history: the age and last author of every line. Docs older than the code they describe are a strong staleness signal.
- This is the "our own idea of the docs" side from P2. It's an inventory of what the program exposes, built without an LLM, so its claims are facts rather than guesses.

**3. Link and compare.**
- **Linking:** claim subjects are matched to entities, exact identifier and path matches first. A cited symbol that doesn't exist fails M5 right away and becomes a stale candidate.
- **Grouping:** comparing every pair of claims doesn't scale, so claims are grouped by shared entity and then by text similarity. Only claims in the same group get compared.
- **Classification:** the model labels each pair as a duplicate, conflict, refinement or unrelated, citing both quotes.
- **The diff against code:** doc claims matched to code facts count as documented. Doc-only claims mean stale docs or unbuilt features. Code-only facts are undocumented features.

**4. Human review.** Every result from step 3 becomes a finding in a queue. Reviewers confirm, reject, merge duplicates, or pick a side in a conflict, with a reason (P3). Confirmed claims become the requirements doc (P1, stage 2).

**5. Distilled docs.** These are rendered only from confirmed claims, one canonical claim per duplicate group, each citing its file, line and quote. The PR commands (M6) turn that into a docs diff.

## Pipeline, not a free-roaming agent

Most of this is a pipeline of small, bounded calls where the model must answer through a tool, not an agent working on its own. That matters for the swappable-model goal. Small structured calls behind deterministic checks work on mid-sized local models; open-ended agents need frontier models. The agent loop is still useful for looking into a single finding, such as "is this claim stale?", with read-only tools: read a file range, search the repo, look up a symbol, check the git history of a line.

## The interface

Drawing the whole graph of an arbitrary program as one picture just gives you an unreadable hairball. Every repo, in any language, has the same four skeletons: the file tree, symbols and references, entry points, and git history. Claims and findings attach to those, and each view starts from one skeleton:

- **Overview:** a treemap of the repo, colored by documentation coverage for code and by finding density for docs. Treemaps work at any repo size, and you can see at a glance where the docs are thin or contradict each other.
- **Review queue:** the main workflow. Findings sorted by severity, with the source spans side by side (doc A vs. doc B, or doc vs. code) and keyboard actions that require a reason.
- **Distilled doc:** the concise output, where each citation opens the source span at its line in a side pane, with a toggle to see the original text.
- **Entity view:** one entity at a time (a service, a route, a concept like authentication), showing every claim about it from docs and code, colored by status (confirmed, conflicting, stale, undocumented). This is the start of the decision tree view (C1). It shows a neighborhood one or two hops out, never the whole graph.
- **Diagrams (R2) later:** a block diagram derived from module structure and imports, with claims layered on top. Fault trees and state matrices come once confirmed requirements exist.

P6 is satisfied because the entities come from each project's own evidence. Only the four skeletons are fixed.

## Questions that shape the next step

1. **Code side of the MVP:** only the deterministic inventory (symbols, routes, commands, config, environment variables), or also LLM-written summaries of each module, cited to code?
2. **First test repo:** Success looks like a good candidate: it has docs, Laravel code, and an AI pipeline we've both read. Or would you rather start with Distillery itself, or Skeleton?
3. **Frontend:** this is still open from the first session. TypeScript has the stronger graph and treemap libraries; Rust compiled to WebAssembly keeps one language.
4. **Entry view:** should the first screen be the overview treemap or the review queue?

---

**Rachel Fish** · 2026-11-13 00:33

1. The goal of the MVP is to be able to show the process of distillation working. Taking the docs from an existing project and making them concise, cited, and meaningful. Get rid of meandering language and obvious AI generated phrasing. Making the docs more concrete, by connecting them directly to the source code. A specific run of the software should also keep track of the current commit hash so all the citations remain accurate for that hash
2. We have lots of repos to test with, I don't think it makes sense to build for any specific one. We are still trying to decide what we are building exactly
3. It may be worth exploring current graph and treemap libraries so we have a baseline to compare typescript against rust. I want to know more about rust compiled to webassembly and your ideas. I think the specific tree visualization is less important for the MVP than simply the data structure itself. Right now, we have to create the links between the docs and the software and create a structure to capture all the decisions in an organization, for example when building a piece of custom hardware that integrates parts from many different vendors. Imagine manufacturing a laptop and the screen, wifi card, RAM, GPU, etc. are all produced by different companies. This is a very common scenario in custom hardware & software projects
4. The first features we build should be functional. I want to be more focused on how to process docs and make decisions on them than view the data in a fancy 3d graphic

---

**Claude**

I'll check the current state of the graph, treemap and Rust/WebAssembly UI libraries so the comparison reflects what exists today.

## Library baseline: TypeScript vs. Rust compiled to WebAssembly

| Need | TypeScript | Rust (WebAssembly) |
|---|---|---|
| UI framework | Mature, many options | Leptos: stable 0.8.22 (released 2026-10-05), with 0.9.0-rc0 released 2026-10-15. Dioxus: 0.7 stable, v0.8.0-alpha.1 in progress |
| Graph view | React Flow suits editable diagrams, Cytoscape.js combines network rendering with graph algorithms, and Sigma.js with Graphology targets larger browser networks | egui_graphs (graph visualization with rust, petgraph and egui in its DNA), drawn on a canvas instead of the DOM |
| Treemap | D3's hierarchy layouts and charting libraries | I didn't find a mature component. A squarified treemap layout is small enough to write ourselves |
| Build cost | Node toolchain | Slow builds: in one test, even a one-button app took more than 4 minutes for its first compile |

The main reason to use Rust for the frontend is that one crate holds the data model (claims, spans, decisions) and compiles into both the server and the browser, so the two can never disagree on types. The cost is pre-1.0 framework churn and thinner visualization libraries.

Since you want functional features first, I'd skip a frontend framework for the MVP. The review queue and the distilled doc are forms, lists and side-by-side text. The server can render them as HTML, with htmx swapping in page fragments. That's a well-trodden stack, where the server returns HTML fragments and HTMX swaps them in the DOM, so you skip a heavy SPA. Axum handles HTTP, Askama renders type-checked templates, with no separate frontend build. You still get one Rust binary, which also suits on-premises installs. The graph view gets decided later, as one page: a Leptos island or an embedded Cytoscape.js. The data model doesn't depend on that choice.

## The data structure

Your laptop example changes what the model has to hold. It isn't "one repo's docs vs. its code" anymore. It's many parties, each publishing versioned sources, with decisions spanning them. Here's the data model as an entity diagram.

[Diagram: data model entity diagram, which failed to render]

The design choices that matter most:

- **Spans are immutable and pinned to a revision.** A span is a revision, a location and the exact quote. For git, the revision is the commit SHA, which covers your point 1: every citation in a run stays accurate for that hash and can render as a permalink (`blob/<sha>/path#L10-L20`). For a vendor PDF, the revision is a content hash plus the vendor's own label (Rev C). Only the location format changes per source type; for a PDF it's page and paragraph instead of line numbers.
- **Claims and decisions outlive revisions; spans don't.** When a new commit or datasheet revision arrives, each span is re-anchored by finding its quote again. Spans whose text is unchanged carry forward automatically. Spans whose text changed flag their claims, and any decisions resting on those claims, for review. A new git commit and a vendor's Rev D are handled by the same mechanism.
- **Entities form a hierarchy, with interfaces as first-class entities.** Laptop → display subsystem → panel, and separately the eDP link between panel and mainboard. Conflicts between vendors almost always sit at interfaces, because that's where two parties each make claims about the same boundary.
- **Authority follows scope.** Each source has an owning party. Vendors are authoritative on their own part specs; the integrator is authoritative on requirements. Decisions within an entity subtree need a reviewer with that scope (P4), so the source of truth (P5) is declared per scope instead of once per project.
- **Decisions are append-only.** A change is a new decision that supersedes the old one, and the old one stays on record.

## The laptop example, run through the model

- **Sources:** the panel vendor's datasheet (Rev C) says the backlight draws up to 4 W. The integrator's power budget doc gives the display 3 W. The firmware repo, at a specific SHA, allows 100% brightness.
- **Claims:** three claims, all about the display power entity, each citing its exact span.
- **Finding:** a conflict at that entity, across three parties' sources.
- **Decision:** the power lead, who has scope over the display subsystem, decides to cap brightness at 75% in firmware. The decision cites all three spans and gives a reason.
- **Later:** the vendor publishes Rev D with a new power figure. The re-anchoring check flags the decision for review.

This is what systems engineering calls requirement allocation, interface control documents, and a traceability matrix. The "state matrix" from R2 is a traceability matrix: requirements down one side, evidence and status across the top. That table is probably the first useful view, and it's a plain HTML table.

## Making docs concise, cited and concrete

- **Nothing disappears silently.** Every extracted claim ends up kept, merged into another claim, or rejected with a reason. The report shows the reduction, from original lines to confirmed claims, and anyone can trace any original sentence to where it went.
- **Phrasing rules are configuration.** A deterministic lint catches filler, hedging and stock AI phrases, using rules each project supplies. The Writing Style section of your guidelines file is already that kind of rule set. The LLM rewrites claim text to pass the lint, and the quote stays attached as evidence, so a reviewer can check that the rewrite kept the meaning.
- **Vague claims get flagged.** Claims with no resolvable subject entity ("the system handles errors gracefully") become a "vague" finding. They either get tied to something concrete in the code or get cut.
- **Code connection:** inline identifiers and paths in the docs link to symbol definitions at the run's SHA. References that don't resolve fail M5.

## Questions

1. **Vendor sources in the MVP:** should the MVP only handle Markdown in git, with PDFs and other source types supported by the data model but not built yet? Or do datasheet PDFs need to work from the start?
2. **Frontend for the MVP:** are server-rendered pages with htmx acceptable, with the Rust-vs-TypeScript decision put off until the graph view?
3. **Cross-scope conflicts:** when a conflict spans two scopes (vendor spec vs. integrator requirement), who resolves it: an owner of the parent entity, or reviewers from both scopes?

Sources:
- [Leptos (0.8.22 / 0.9 RC status)](https://www.issoh.co.jp/md/tech/9089)
- [Dioxus releases](https://github.com/DioxusLabs/dioxus/releases)
- [egui_graphs](https://github.com/blitzarx1/egui_graphs)
- [React Flow vs Cytoscape.js vs Sigma.js](https://www.techbloat.com/?p=1964699)
- [HTMX + Axum + Askama starter](https://railway.com/deploy/htmxrustaxumaskamapostgres)

---

**Rachel Fish** · 2026-11-13 00:42

[Attached images: screenshot of the data model diagram failing with a Mermaid syntax error, screenshot of the pipeline diagram]

A couple things here. 1. there was a syntax error in your mermaid diagram and it failed to load.

2. we are getting lost in the weeds here, we have to stay focused. I liked the pipeline diagram you presented earlier, that was a great concise analysis. We need more of that. less walls of text

---

**Claude**

Sorry about the broken diagram. Here's the data model again in the same style as the pipeline.

[Diagram: Distillery data model]

- **Spans:** fixed to one commit SHA or document revision, so every citation stays accurate for that run.
- **New revisions:** quotes are found again in the new version. Unchanged spans carry forward; changed spans flag the claims and decisions that rest on them.
- **Interfaces:** stored as entities, because conflicts between vendors happen where two parts meet.
- **Decisions:** never edited. A change is a new decision that supersedes the old one.

I'll keep things this concise from here on. Three open questions:

1. Should the MVP read only Markdown from git, or also vendor PDFs?
2. Are server-rendered pages acceptable for the MVP, with the graph view decided later?
3. Who resolves a conflict between scopes, such as a vendor spec against your own requirement?

---

**Rachel Fish** · 2026-11-13 14:32

1. MVP only reads markdown, we should iterate and quickly follow up with PDF processing after the MVP is complete.
2. Yes we can render everything server-side in rust and present HTML & javascript based on the processed data
3. Users of the software resolve conflicts & make decisions, with an audit trail of the decisions and reasons why people made specific decisions with cited sources. In the same way that we can track docs & source code with citations, we can also have citations for decisions explaining, with references to specific meetings & discussions and explanations of why specific technical decisions are being made within a project (Much of this is out of the scope of the MVP, but I've provided a description here for context of what we're building towards)

---

**Claude**

That settles the MVP scope:

| Area | MVP | Next |
|---|---|---|
| Input | Markdown in a git repo, pinned to one SHA | PDFs |
| Code links | Cited symbols exist at the cited lines | Checking what the code actually does |
| Interface | Server-rendered HTML and JavaScript from Rust | Graph views |
| Decisions | Made by users, with a reason, citing the finding's spans | Reasons that cite meetings and discussions |

Your third answer means one change to the model: decisions cite spans, the same way claims do. That gives one citation mechanism for everything. Docs, code, transcripts and the reasons behind decisions all point at exact quotes in pinned revisions. In the MVP a decision cites the spans of the finding it resolves, plus the reviewer's written reason. Later, the reason itself can cite a meeting transcript or discussion thread.

For the next step, I'd pick one of these:

1. **Run flow:** what happens from pasting a repo URL to a finished run, including jobs, progress and failures.
2. **Claim extraction:** the tool schema and prompt for turning one Markdown section into verified claims. This is the core of distillation.
3. **Review screens:** the pages a reviewer works through, and the actions on each.

I'd suggest starting with claim extraction, since the quality of the distilled docs depends most on it.

---

**Rachel Fish** · 2026-11-14 02:21

Agreed, claim extraction seems like the most sensible starting point. That is the essence of docs distillation. Let's capture these diagrams and updated decisions in the project documentation and create a technical implementation plan for the MVP. Like the previous session, write the transcript file last such that your response in the chat is recorded in the transcript for future review

---

**Claude**

I'll start by reading how the previous session's transcript, decisions and README are laid out so this commit follows the same format.

I have the format. Next I'll set up a working copy and write this session's transcript, up to your latest message, so the new decisions can cite its line numbers.

Now the decisions source. It uses citation keys, and a script turns them into numbered references after checking every quote against the transcripts.

The first session's transcript repeats a few of your quotes, so for existing references I'll verify the quote sits on the line already cited, and keep the strict exactly-once check for this session's new ones.

All 71 citations check out. Next, the plan doc with the two diagrams as Mermaid, using only basic flowchart syntax so it renders on GitHub.

Now the quick checks: the JSON schema parses, every link between docs resolves, and no Markdown code block is missing a language tag.

The only broken links point to this session's transcript, which isn't written yet. I'll write it now, ending with this reply.

[Attached files: transcripts/2026-11-12-design-session-3.md, docs/decisions.md, docs/mvp-plan.md, README.md]

This commit adds this session's transcript, the new decisions, and an MVP plan with both diagrams. All 71 citations appear on the lines they cite, the JSON schema parses, and every link resolves.

**Open the files (same order as the attached files):**

```bash
codium transcripts/2026-11-12-design-session-3.md
codium docs/decisions.md
codium docs/mvp-plan.md
codium README.md
```

**What changed:**
- **Decisions:** 21 new citations from this session. New decisions: C4 (decisions across an organization and its vendors), M2 (MVP goal), M3 (claim extraction first), M6 (runs record the commit hash), M10 to M13, R7 (docs written once from the agreed state), I3 and I4 (Rust agent loop, on-premises hosting), L1 (PDFs next) and L5 (decisions cite meetings). I2, M5 and P3 are extended.
- **MVP plan:** the pipeline and data model as Mermaid diagrams, the claim extraction tool schema, model access, the stack, a nine-step build order and open questions. Everything in it stays a proposal until confirmed in the decisions file.
- **Transcript:** this session word for word, with diagrams noted by name, ending with this reply.
- **README:** links the plan.

**QA steps:**
1. Open `docs/mvp-plan.md` on GitHub and check that both diagrams render. I couldn't render Mermaid here, and the last one I made failed.
2. Read the new and extended decisions against your words in the transcript.
3. Spot-check a few references and links.

**Things to check:**
- **Code side of the MVP:** the plan limits code facts to identifiers and paths at the SHA (M8), and leaves generating docs from code (P2) for later. You didn't answer that question directly.
- **Claim kinds:** the six kinds in the schema are my guess, and the plan lists them as an open question.

**Commit message:**

```text
docs: add MVP plan with pipeline, data model and claim extraction
```

---

**Rachel Fish** · 2026-11-16 13:43

Before we commit these changes, I would like to discuss some of the details. Why did you decide to use `sqlx` over `rusqlite`? I would prefer to use `rusqlite` for consistency with my other projects for now, and if we want to use a relational database we should consider our options for a ORM instead of just sqlite.

I don't want the planning docs to have any open questions. Let's discuss everything in detail first, so we can hand off a concise development plan when we are done here.

---

**Claude**

I didn't have a strong reason for `sqlx`. It carried over from my note in the first session that sqlx supports both SQLite and Postgres, which would make a later move easier. It was never your decision, so I'll switch the plan to `rusqlite`.

On ORMs, these are the options that matter for a later move to a server database:

| Option | Fit |
|---|---|
| Raw SQL on `rusqlite` | What the plan will use. Queries stay in one storage module, so a later swap only touches that module. |
| Diesel | A safe, extensible ORM and Query Builder for PostgreSQL, SQLite, and MySQL, with queries checked at compile time. It uses its own SQLite bindings rather than rusqlite, so moving to it would mean rewriting the queries. |
| SeaORM | 2.0 went stable in July 2026. The sea-orm-sync crate provides a synchronous SeaORM backed by rusqlite, mirroring the async API with async/await stripped away. That makes it the closest step up from rusqlite, and the async version covers Postgres. |

My recommendation is raw `rusqlite` for the MVP, kept behind a storage module. Choose an ORM when a second database is actually needed, with SeaORM's sync crate as the first one to evaluate.

To leave no open questions, here's everything still unconfirmed, with my recommendation for each. Reply with the numbers you'd change; I'll treat the rest as agreed.

| # | Item | Recommendation |
|---|---|---|
| 1 | Storage | `rusqlite` with raw SQL behind a storage module; ORM chosen later, as above |
| 2 | Code side of the MVP | Only check identifiers and paths at the SHA (M8). Generating docs from code (P2) comes after the MVP |
| 3 | Claim kinds | Behavior, configuration, requirement, decision, instruction, rationale ("how-to" renamed to instruction) |
| 4 | Claim size | One statement a reviewer can accept or reject on its own |
| 5 | Tables | One claim per row, quoting that row |
| 6 | Code blocks | Not claims themselves. Commands become instruction claims quoting the command line, and identifiers inside code blocks are checked like inline code |
| 7 | Run flow | One run at a time, as a background task in the server. Progress is saved per file and the page polls for it. A failed section is logged and the run continues; a re-run skips cached sections |
| 8 | Model adapters | Provider trait plus the Anthropic adapter in the MVP. The OpenAI-compatible adapter comes first after the MVP, alongside PDFs |
| 9 | Web stack | axum, Askama, htmx |
| 10 | Markdown parser | pulldown-cmark, which reports the source offset of every element |
| 11 | Git | The `git` CLI (clone, rev-parse, ls-files), not a library |

Sources:
- [Diesel](https://docs.rs/crate/diesel/latest)
- [SeaORM 2.0.0 release notes](https://newreleases.io/project/github/SeaQL/sea-orm/release/2.0.0)

---

**Rachel Fish** · 2026-11-16 13:56

This is a great example of exactly the type of problem I want to capture via this project. Key technical decisions can be made through assumption which snowball into specs and documentation without anyone actually signing off on the decision. You noted the possibility of using sqlx in the past, which then turned into a recommendation, which would have then turned into an official planning document if I had not flagged it.

That is the exact sequence of events that distillery should be designed to catch: where an assumption turned into a decision that nobody actually decided on.

Agreed, let's use rusqlite for the MVP and come back to this in the future once we have a better idea what our final data model will be and once we've had time to consider different ORM implementations.

Regarding the recommendations:

1. Confirmed, rusqlite is sufficient for a prototype, but we should note that this is not a final decision for the rest of the project, we may want to consider other options in the future once we've built out more than a simple prototype
2. Can you expand on this? I want to better understand what you are describing before I agree to anything.
3. Let's expand this with examples so I understand what you mean a bit better
4. Claims can be nested. We want to be able to break claims down into their constituent parts, but we have to think about this like atoms and molecules. Some claims are atomic, they are the minimal decision set within a system. Other claims may depend on other decisions, these are more like molecules - claims that depend on each other, claims that have some kind of relationship to each other. This needs to be captured in the data structure
5. Tables? You mean like how do we process data within a tabular format? Once again, can you provide me with some examples of what you mean here?
6. Not sure what you are suggesting here either
7. Let's expand on this as well. You're describing a failure condition where part of the docs cannot be distilled?
8. Yes let's design this such that we are platform agnostic
9. Why Askama? Why not maud?
10. `pulldown-cmark` is approved
11. Yes, let's use native git commands where possible

---

**Claude**

Agreed, and this conversation is a good test case. The chain went from my passing note (session 1), to a recommendation (session 3), to a line in the plan, and nobody confirmed it at any step. To catch that, the data model needs two things:

- **An author on every claim,** with AI agents as parties like people are.
- **A trace for every decision** back to a person confirming it. A decision whose support ends at a proposal gets flagged as an unconfirmed assumption.

The plan doc already labels everything in it as a proposal; the handoff version will also say who proposed each item.

**1. Storage:** recorded as `rusqlite` for the prototype only, to be revisited once the data model settles and ORMs have been compared.

**2. Code side of the MVP:** take a doc line like "`ResumeAiService::generateDraft` builds the draft":
- **MVP:** `git grep` at the run's SHA confirms that `generateDraft` exists and links to its file and line. Paths like `docker/php/custom.ini` get the same check.
- **Not in the MVP:** checking that the method actually builds the draft, and reading the code to list what the docs never mention, such as undocumented routes (P2).
- **Limit:** this is a text search, not code understanding. It can tell that a name exists, but not what it does.

**3. Claim kinds**, using lines from your guidelines file:

| Kind | Example |
|---|---|
| Requirement | "Always use an explicit language identifier on every code block" |
| Rationale | "Bare ``` without a language tag causes consecutive blocks to merge" |
| Behavior | "The Claude interface silently drops the rest" (past 20 artifacts) |
| Instruction | "Run a syntax check on every changed file: `php -l` for PHP" |
| Decision | "Use rusqlite for the MVP" (from this conversation) |
| Configuration | A setting and its value, such as an environment variable's default |

**4. Atoms and molecules:** claims link to other claims, with **part of**, **depends on** and **supports** alongside duplicate, conflict and supersedes. Here's the "Presenting Changes" section of your guidelines broken down that way.

[Diagram: claims as atoms and molecules]

- **Atoms** quote one span. **Molecules** name their parts, and can draw on several sections or files.
- **Rejecting an atom** flags every molecule and dependent claim built on it. That's the same check that would have caught `sqlx`.

**5. Tables:** I mean Markdown tables in the docs being distilled.
- **Fact tables,** such as `| Variable | Default |` with a row like `| APP_PORT | 8080 |`. Each row becomes an atom ("`APP_PORT` defaults to 8080") quoting that row, and the table becomes a molecule.
- **Example tables,** like the "Instead of / Write" table in your guidelines. The rows illustrate a rule rather than state facts, so the table supports the writing-style requirement instead of becoming claims.

**6. Code blocks:** fenced code in the docs being distilled.
- **Commands,** such as `php artisan test --filter=AlertHistoryServiceTest`, become instruction claims quoting the command.
- **Code samples** aren't claims, but names in them get the item 2 check: does `AlertHistoryServiceTest` exist at the SHA?
- **Example blocks** are marked as examples so they don't get flagged. Your guidelines' `codium laravel/app/Http/Controllers/ExampleController.php` is illustrative, and shouldn't count as a missing file.

**7. Failures:** yes, parts of the docs that can't be distilled in one run.
- **Provider errors** (timeouts, rate limits) are retried with backoff.
- **Claims that fail the quote check twice** leave their section marked "not distilled," with the rejected output shown.
- **Sections too long for the model's context** are split at subheadings, then at paragraphs.
- **The report** shows something like "12 of 14 sections distilled, 2 need attention," and a re-run only retries those two. Nothing is dropped silently.

**8. Platform agnostic:** confirmed. One question: should the MVP ship both adapters, Anthropic and OpenAI-compatible? I'd include both, since a second adapter is what proves the trait is really platform agnostic.

**9. Askama vs. maud:** no real reason; Askama was my default, which is the same pattern as `sqlx`. Both are checked at compile time. Askama keeps HTML in separate template files. Maud writes HTML as Rust macros, so pages are plain Rust functions with no template files. For a Rust-only team, maud fits well. Do you want maud?

**10–11:** recorded as approved.

---

**Rachel Fish** · 2026-11-16 14:15

2. Does it make sense to do this type of introspection using git grep when we aren't even processing the code for logical behavior? In my opinion we should be focused on extracting the essence of the documentation, the claims, decisions, and the user interface before we try doing anything like automatically checking the code. We want to make sure we are not biting off more than we can chew. We should be making small, surgical, isolated changes as we iterate through this so we can iterate quickly instead of getting lost in scope creep.
3. This is a good starting point, but I'd be curious if there are any other projects that we can use for inspiration to determine what would be appropriate? Once again, let's not spend too much time on this, I think this is a good preliminary starting point. The most important part of an MVP is actually building it so we can iterate and try out different ideas.
4. This is great, I like this way of thinking and framing the problem space
5. Yes in this situation, the cited table is providing context for illustrating a rule, however in other projects tables in the documentation could hold all sorts of information. For example, findings, simulation output, hardware choices, etc. We shouldn't expect data to always follow a certain format
6. You're becoming hyperfocused on the AI development guideliens doc that I provided you instead of thinking about project documentation in a more general categorical sense.
7. We definitely want to handle incremental distillation and failures. For example, some projects might have hundreds of docs files but we only want to process a few of them at a time to make sure the program is working
8. Let's keep things simple and only ship with Anthropic API for now. I already have an Anthropic API key and I don't want to set up a ChatGPT API key just for testing this MVP. Once we have more features built it will make more sense to test other platforms.
9. Let's use maud for consistency across other projects and simplicity in building out the MVP

This should be enough context to build a final version of the planning documentation for review

---

**Claude**

I'll take a quick look at existing frameworks for categorizing documentation and decisions to inform the claim kinds. Then I'll write the final plan, the decisions and the transcript.

[Attached files: transcripts/2026-11-12-design-session-3.md, docs/decisions.md, docs/mvp-plan.md, README.md]

Here's the final plan for review, with the decisions and transcript updated to match. All 93 citations appear on the lines they cite, both JSON schemas parse, and every link resolves.

**Open the files (same order as the attached files):**

```bash
codium transcripts/2026-11-12-design-session-3.md
codium docs/decisions.md
codium docs/mvp-plan.md
codium README.md
```

**What changed since the last draft:**
- **Plan:** no open questions. Code checks are out of the MVP. Extraction runs in two passes, atoms and then molecules and links. Runs process selected files and resume after failures. The stack is rusqlite, maud, pulldown-cmark and native git.
- **Claim kinds:** drawn from FActScore (atomic facts), Diátaxis (doc types) and IBIS (positions, and arguments that support or oppose them), summarized in a three-row table in the plan.
- **Decisions:** 22 new citations. New decisions: C5, M4, M5, P4 to P6, R8, I6 to I8, L2 and L3. M6, M8, M12 and M13 are extended.
- **README:** no longer says the first version links facts to source code.
- **Transcript:** adds today's discussion, ending with this reply.

**Superseded by your latest answer:** in session 2 you asked for "clickable citations that bring you to the source code" and for the MVP to "check that the specific functions exist at the cited lines." Both conflict with keeping code checks out of the MVP, so M6 now says MVP citations open the docs, and both moved to L2. Please confirm.

**Sign-off:** everything in the plan without a decision number is my proposal, and approving the plan signs it off. These deserve the closest look:
- axum with tokio for the web server
- Plain JavaScript with no framework (htmx dropped)
- reqwest for the Anthropic API, with the key from `ANTHROPIC_API_KEY`
- Schema versioning with SQLite's `user_version`
- Two-pass extraction, the six link types, and the `illustrative` flag for examples in docs

**QA steps:**
1. Check that both diagrams render on GitHub.
2. Read the plan, looking for anything without a decision number that you wouldn't sign off.
3. Spot-check a few new references against the transcript.

**Commit message** (replaces the previous one, still one commit for all four files):

```text
docs: add MVP development plan and claim extraction decisions
```
