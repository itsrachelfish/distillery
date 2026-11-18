# Decisions

The current decisions. Each section numbers its own decisions with its letter, most important first. The numbers change whenever a section is reordered, so they aren't permanent IDs. Each decision cites its sources by number. The references at the bottom give the file, line and exact quote. Rachel Fish reviews and commits every revision, and `git log -p docs/decisions.md` shows every earlier version. [docs/mvp-plan.md](mvp-plan.md) is the development plan built on these decisions.

## C: What Distillery is

- **C1.** A decision engine: a visual interface where people see programs as decision trees and the information relationships behind them. [1] [2]
- **C2.** Goal: distill projects to their essence, prove every fact, and build a network of decisions that forms the spec. [3]
- **C3.** Docs and transcripts become concise and cited to real decisions, so people can agree on the current state of tribal knowledge, and so slop and hallucinations don't get in. [4] [5]
- **C4.** A structure for every decision in an organization, including custom hardware that integrates parts from many vendors. [6] [7]
- **C5.** Catches assumptions that turned into decisions nobody actually made. [8]

## M: MVP

- **M1.** The MVP distills docs, as a proof of concept for the visual decision tree and requirements system. v1, described in the README, is the end state after iterating; the MVP is its first step. [9] [95]
- **M2.** Goal: show distillation working. Docs become concise, cited and meaningful, without meandering language or obvious AI phrasing. [10] [11]
- **M3.** Claim extraction is built first, as the essence of docs distillation. [12]
- **M4.** The focus is the essence of the docs: claims, decisions and the user interface. Nothing checks the code automatically. [13]
- **M5.** Built in small, isolated changes, to iterate quickly without scope creep. Building it and trying ideas matters more than planning every detail. [14] [15]
- **M6.** A Rust web server: the user pastes a git repo URL, the analysis runs, and the distilled docs appear with clickable citations. In the MVP, citations open the docs they came from; citations into the source code come later (L2). [16] [13] [94]
- **M7.** Input: only the Markdown docs in a repo that also holds the code. [17] [18] [19]
- **M8.** Each run reads one snapshot and records its commit hash, so citations stay accurate for that hash. Runs can process a few files at a time, and distillation is incremental and handles failures. [20] [21] [22] [23]
- **M9.** Every stage reports progress on the command line and in the web interface: what is being processed, record counts, the current item, and a progress bar or percentage. The interface explains each step and never appears frozen. [96] [97] [98]
- **M10.** It finds stale information, redundancy, conflicting claims and needless detail, and has an interface for human review. [18] [24] Staleness can't be judged until docs are checked against code (L2); until then, git history can show which of two conflicting docs is newer. [120] [131]
- **M11.** Output: the distilled docs, plus commands for the user to create a PR that updates the docs. [25]
- **M12.** Functional features come first: processing docs and making decisions on them. The data structure matters more than the visualization. [26] [27]
- **M13.** Pages are rendered server-side in Rust with maud and served on localhost only, since the MVP has no login. Plain HTML, with no JavaScript or keyboard shortcuts until a page needs them. [28] [29] [99] [100] The server rejects every non-local request. [121]
- **M14.** Uses only the Anthropic API, through its REST API rather than an SDK, behind a platform-agnostic design. [30] [31] [32] [101]
- **M15.** What counts as one distilled fact is worked out by trying the process on real projects, without building for any specific repo. [33] [34]
- **M16.** Settled for the MVP: sections, extraction schemas, validation, runs and failures, and the build order. The data model, claim kinds and link types are starting points, iterated during development. [102] [103]
- **M17.** The web interface is built before sections and claim extraction, so each later step is checked in the browser as it is built. [137] [138] [139]
- **M18.** Every model API call is recorded with its tokens and cost, and runs show an estimate before they start, as Wetfish Success does. Runs stop at a cost limit. [141] [142]
- **M19.** Runs are synchronous to start. Processing docs files in parallel comes later, where it makes whole-repo jobs finish faster. [124]
- **M20.** Prompt caching and extended thinking are out of scope for the MVP. Each needs a trade study of its actual impact. [144]
- **M21.** Section rules are a proof of concept, open to change when testing shows problems. Splitting long sections is decided from real results. [147] [148]

## P: How distillation works

- **P1.** Stages, each feeding the next: distill the docs; reviewers confirm facts against their own knowledge, which builds a requirements doc; check the code against the requirements; distill the code to its functional parts; improve implementations and data structures; finish with a security audit that proves everything works as intended and allows no unauthorized access. [35] [36] [37] [38] [39] [40]
- **P2.** Docs generated from the code are part of the distillation report, including for legacy projects with no docs. The same process runs on projects with and without docs, so docs generated from the code alone can be compared with what the docs say. [41] [42]
- **P3.** Every decision is confirmed by a person, who is recorded. Users resolve conflicts and make decisions, and the audit trail records each decision, the reason for it and its cited sources. [43] [44] [45] Rejections need a reason; confirmations can omit it. [118]
- **P4.** Records written by AI agents are flagged as AI proposals until a person reviews them, so assumptions never become documented requirements without human review. [104]
- **P5.** Claims nest. Atomic claims are the minimal decisions in a system; compound claims are made of, or depend on, other claims. The data structure captures both. Atoms and molecules were an illustration, not names. [46] [47] [48] [105] [106]
- **P6.** Docs content is processed without assuming a format. Tables, for example, can hold findings, simulation output or hardware choices. [49] [50]
- **P7.** Claim kinds start from a preliminary set: behavior, configuration, requirement, decision, instruction and rationale. [51]
- **P8.** Reviewers approve decisions for specific departments, sections of a project, or repos. [52]
- **P9.** Every analysis starts with the user declaring the source of truth. [53] Declaring it means pointing to the places in source code or meeting transcripts that record a decision. Out of scope for the MVP's first iteration. [125] [126]
- **P10.** Each project's structure is discovered from its own evidence, never assumed from another project. [54]

## R: How decisions are recorded

- **R1.** This project's transcripts are its source of truth. [55] They are kept while the project is being built. By the end, they are distilled into only the important information, held in the project's own data structure with decisions and their reasons, and removed from the repo. [151] [152]
- **R2.** The current state can be viewed as a diagram, such as a block diagram, fault tree or state matrix. [56]
- **R3.** Each section numbers its decisions with its own letter, ordered by priority. The numbers make decisions easy to reference, but they change and aren't IDs. Citations are auditable, and earlier decisions and changes stay on record. [57] [58] [59] [60]
- **R4.** Decisions cite numbered references to a file, line and exact quote. Transcripts, docs and source code are cited the same way. [61] [62]
- **R5.** Conventional Commits 1.0.0, in this project and in the PRs and issues Distillery creates for other projects. [63] [64]
- **R6.** Code changes and conversations are kept as concise as possible. [65]
- **R7.** Docs are written once, from the final agreed state, instead of being rewritten along the way. [66]
- **R8.** Planning docs have no open questions. Details are settled in discussion first, then handed off as a concise development plan. [67] [68] Handoffs carry no open questions either. [149]
- **R9.** Writing describes categories in short noun phrases, not single objects or imagined examples, and cuts fluff. Distillation applies the same rules, even to a short README. [107] [108]
- **R10.** Terms are defined once, in a small table in the MVP plan, with no separate glossary doc. [103] [109] [110]
- **R11.** Development follows the AI development guidelines' workflow, on a feature branch: changes are proposed in chat before drafting, iteration commits carry manual QA steps, and tests and docs are updated at the end of the feature. [136] [127] [140] [146]
- **R12.** Agents skip questions with obvious answers, so iteration stays quick. [135]
- **R13.** Intermediary files, such as handoffs, stay out of the repo. [150]

## I: Implementation

- **I1.** Written in Rust. [69]
- **I2.** Supports multiple LLM platforms, and each project can choose Claude, ChatGPT or other models. The Claude API is the benchmark. [70] [71]
- **I3.** Distillery's Rust server runs the agent loop, so the model is easy to swap, and no vendor's agent SDK locks it in. [72] [73]
- **I4.** Designed for on-premises hosting soon after the MVP, which opens Distillery to a much broader set of problems. [74] [75]
- **I5.** LLMs distill docs. Code analysis uses traditional tools wherever possible; the specific tools are still to be chosen. [76] [77]
- **I6.** Storage is rusqlite for the prototype, set up like web-of-trust: schema steps tracked by SQLite's `user_version`, with no migration crate. This isn't final: ORMs get compared once the data model settles. [78] [79] [111] Each build step adds the tables it needs: no table exists for features not yet built. [119]
- **I7.** Markdown is parsed with pulldown-cmark. [80]
- **I8.** Native git commands wherever possible. [81] Repos are cloned bare with full history, which conflict review needs, and no hooks run. Clones and fetches show git's own progress output. [132] [129] [130] [134]
- **I9.** axum with tokio for the web server. [99]
- **I10.** reqwest for model API calls. [101]
- **I11.** Settings live in a TOML config file, laid out like lurkers' `lurker.example.toml`: reviewer, model, port, database path and clone folder. Repositories are user input, added through the web interface. [112] [113] [114] [115] [116] The model API key, model prices and the run cost limit are settings too; no environment variables. [143] [145]
- **I12.** Prompts and tool schemas are tracked in git. The prompt version is their hash, identifying records that need re-running. [117]
- **I13.** clap for the command line. [128]
- **I14.** Tests sit beside the code and are run by the developer, as in other Wetfish projects. [123]
- **I15.** Model API errors are handled, including replies cut off at the token limit and refusals, which strict tool use doesn't cover. [122]

## L: Later

- **L1.** PDF processing, right after the MVP, then other doc formats such as plain text and HTML. The prototype reads Markdown files only. [19] [133]
- **L2.** Connect the docs to the source code and check them against it, starting with whether cited functions exist at the cited lines. [11] [82] [13]
- **L3.** Other model platforms, once more features are built. [83]
- **L4.** Code analysis supports at minimum PHP, Rust, Python, JavaScript (including Node.js) and Go, starting with PHP. [84] [85] [86]
- **L5.** Running project test suites, coverage, mutation testing and performance analysis. [87]
- **L6.** Distill meeting transcripts in corporate settings into only the most important decisions: newest first, who said what, and who the responsible engineers are. [88] [89]
- **L7.** Decisions cite the meetings and discussions that explain why technical decisions were made. [90] [91]
- **L8.** Run in CI: flag PRs that conflict with confirmed requirements, and only merge code that has cited documentation. [92]
- **L9.** Open PRs, create issues, and integrate with Linear. [93]

## References

1. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 263: "We are creating the decision engine interface. A visual interface so people can conceptualize programs as decision trees"
2. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 138: "We are building an information relationship visualization interface"
3. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 368: "building up a network of decisions that creates a spec of what is supposed to be"
4. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 265: "we want to make our docs & transcripts more concise, so it's easier for people to coordinate on the current state of tribal knowledge"
5. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 275: "making docs concise and cited to real decisions is the best way to prevent this kind of slop and hallucinations"
6. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 223: "create a structure to capture all the decisions in an organization, for example when building a piece of custom hardware that integrates parts from many different vendors."
7. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 223: "This is a very common scenario in custom hardware & software projects"
8. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 457: "That is the exact sequence of events that distillery should be designed to catch: where an assumption turned into a decision that nobody actually decided on."
9. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 259: "D14 - MVP distills docs. Proof of concept for the visual decision tree & requirements specification system."
10. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 221: "The goal of the MVP is to be able to show the process of distillation working. Taking the docs from an existing project and making them concise, cited, and meaningful."
11. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 221: "Get rid of meandering language and obvious AI generated phrasing. Making the docs more concrete, by connecting them directly to the source code."
12. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 353: "Agreed, claim extraction seems like the most sensible starting point. That is the essence of docs distillation."
13. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 536: "In my opinion we should be focused on extracting the essence of the documentation, the claims, decisions, and the user interface before we try doing anything like automatically checking the code."
14. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 536: "We should be making small, surgical, isolated changes as we iterate through this so we can iterate quickly instead of getting lost in scope creep."
15. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 537: "The most important part of an MVP is actually building it so we can iterate and try out different ideas."
16. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 295: "Rust webserver, starts up, user pastes git repo into the web interface, analysis runs, docs generated, displayed in web interface with linked clickable citations that bring you to the source code"
17. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 289: "Markdown within a repo is fine for docs processing for now"
18. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 293: "Our MVP should simply accept a git repo with code & docs inside and then distill the docs in it to look for stale information, redundancy, conflicting claims, and an interface for human review."
19. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 322: "MVP only reads markdown, we should iterate and quickly follow up with PDF processing after the MVP is complete."
20. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 240: "But for right now, a single analysis run with a report."
21. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 221: "A specific run of the software should also keep track of the current commit hash so all the citations remain accurate for that hash"
22. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 541: "We definitely want to handle incremental distillation and failures."
23. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 541: "some projects might have hundreds of docs files but we only want to process a few of them at a time to make sure the program is working"
24. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 275: "many AI generated documents are very long with needless details."
25. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 261: "D16 - Both. Display the distilled docs and display the commands for a user to create a PR to update the docs"
26. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 224: "The first features we build should be functional. I want to be more focused on how to process docs and make decisions on them than view the data in a fancy 3d graphic"
27. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 223: "I think the specific tree visualization is less important for the MVP than simply the data structure itself."
28. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 323: "Yes we can render everything server-side in rust and present HTML & javascript based on the processed data"
29. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 543: "Let's use maud for consistency across other projects and simplicity in building out the MVP"
30. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 77: "We can use the claude API for now for the MVP"
31. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 542: "Let's keep things simple and only ship with Anthropic API for now."
32. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 470: "Yes let's design this such that we are platform agnostic"
33. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 290: "I think this should come from us analyzing a project and figuring this out by trying the process"
34. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 222: "We have lots of repos to test with, I don't think it makes sense to build for any specific one."
35. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 170: "Once we have a concise view of "this is what the docs say" a reviewer can compare the distilled documentation against their own working knowledge."
36. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 170: "As details are confirmed during review, the system builds out a comprehensive requirements doc."
37. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 170: "Then these requirements feed back into the program and allow us to do another iteration: checking what the code code does against what it is supposed to do."
38. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 170: "Then, once we have reviewed that, we are able to distill the code to only the functional parts."
39. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 170: "are these implementations ideal? are there more efficient ways of doing this? are there better data structures to accomplish this goal?"
40. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 170: "we can do a final security audit iteration where we prove that everything works as intended and does not allow for unauthorized access"
41. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 273: "N1 - Docs generation should be a part of the distillation report, and we need to work in docs generation for projects which have no docs already, like old legacy codebases."
42. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 253: "We can use the same techniques on a project with documentation and without, and compare what we generate in a clean implementation (reading the code, no docs) versus reading what the docs say."
43. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 268: "Every decision should be confirmed, and it should say by who."
44. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 292: "a reviewer has to confirm specific facts & decisions and pick sides for conflicts, ideally explaining why so other team members can understand the reasoning behind decisions and scope changes"
45. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 324: "Users of the software resolve conflicts & make decisions, with an audit trail of the decisions and reasons why people made specific decisions with cited sources."
46. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 466: "Claims can be nested. We want to be able to break claims down into their constituent parts, but we have to think about this like atoms and molecules."
47. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 466: "Some claims are atomic, they are the minimal decision set within a system. Other claims may depend on other decisions, these are more like molecules - claims that depend on each other, claims that have some kind of relationship to each other. This needs to be captured in the data structure"
48. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 538: "I like this way of thinking and framing the problem space"
49. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 539: "We shouldn't expect data to always follow a certain format"
50. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 539: "in other projects tables in the documentation could hold all sorts of information. For example, findings, simulation output, hardware choices, etc."
51. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 537: "I think this is a good preliminary starting point."
52. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 268: "each person could be reviewing and approving decisions for specific departments, sections of the project, specific repos, etc."
53. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 136: "The user says what the source of truth is. When we start analyzing a new project we always need to know: what is the source of truth?"
54. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 366: "Copying the structure of this single repo will not help you analyze the structure of future repos"
55. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 269: "Transcripts are the source of truth for how we are creating this project"
56. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 285: "We need to have the current source of truth, something people can look at, like a block diagram, a fault tree, a state matrix."
57. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 285: "The references and citations it's built on should be auditable, with a record of all previous decisions and changes. I think the "decisions" list should be mutable, with decisions ranked by priority"
58. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 285: "doesn't pollute people's mind with random "decision ID numbers" that inevitably become invalidated"
59. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 361: "it's fine to use a numbered list for the decisions, so that it's easier to reference them, but we should recognize that the numbers are mutable and not set in stone, they are ordered by priority"
60. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 398: "I think each section should have its own prefix"
61. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 412: "provide htis conversation as a transcript in the distillery project and then mention citations referencing the transcript."
62. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 412: "In this way, a transcript, a docs file, or lines of source code can all be used interchangeably"
63. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 466: "include the use of Conventional Commits, https://www.conventionalcommits.org/en/v1.0.0/"
64. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 468: "I want to ensure that we are following these best practices as we build this project and as we create PRs & issues for other projects."
65. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 309: "I want us to keep our code changes and dialog exchanges as concise as possible"
66. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 145: "I think it's better to update the docs once at the end with our final agreed upon state instead of incrementally rewriting it multiple times."
67. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 413: "I don't want the planning docs to have any open questions."
68. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 413: "Let's discuss everything in detail first, so we can hand off a concise development plan when we are done here."
69. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 124: "but I want this project itself to be written in Rust"
70. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 128: "for now I want to develop this in an architecture where we can use other platforms, but for now just using the Claude API as a benchmark."
71. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 79: "Different projects might want to use claude, chatgpt, or other models."
72. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 79: "I think rust running the agent loop makes the most sense, as it allows the model running the loop to be swapped out easily."
73. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 79: "If so, my worry with that would be vendor lock-in."
74. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 77: "we have to design this in such a way that we can do our own on-premises hosting sooner rather than later."
75. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 77: "That opens the project up to run on a much broader set of problems."
76. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 265: "docs distillation makes sense to use LLMs, but we have to start specifying what types of tools and the specific implementation we'll be using for code analysis."
77. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 134: "we actually avoid using LLMs and use traditional code analysis tools as much as possible"
78. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 459: "Agreed, let's use rusqlite for the MVP and come back to this in the future once we have a better idea what our final data model will be and once we've had time to consider different ORM implementations."
79. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 463: "Confirmed, rusqlite is sufficient for a prototype, but we should note that this is not a final decision for the rest of the project"
80. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 472: "`pulldown-cmark` is approved"
81. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 473: "Yes, let's use native git commands where possible"
82. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 291: "For an MVP I think we should just check that the specific functions exist at the cited lines, but in production we need to do full code analysis and implementation"
83. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 542: "Once we have more features built it will make more sense to test other platforms."
84. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 124: "We need to be as language agnostic as possible, I want to support projects written at a minimum: PHP, Rust, Python, JavaScript (& Node.js), Go."
85. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 124: "We can start with PHP as a starting language to process"
86. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 267: "D5, D6 - Agreed, this is only important for code analysis, which we can defer for now."
87. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 170: "This should be a long term goal, not an immediate need."
88. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 269: "ultimately what we will be distilling in corporate settings that have many transcribed meetings and we need to extract only the most important decisions from them"
89. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 127: "Meeting transcripts, ordered by recency, specifically who said what."
90. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 324: "we can also have citations for decisions explaining, with references to specific meetings & discussions and explanations of why specific technical decisions are being made within a project"
91. [transcripts/2026-11-12-design-session-3.md](../transcripts/2026-11-12-design-session-3.md) line 324: "Much of this is out of the scope of the MVP"
92. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 238: "We can automatically flag things like where someone submits a PR with code that conflicts against the confirmed requirements for the project, and only allow new code to be merged when it has cited documentation."
93. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 129: "Ideal end state: PRs against the repo, issues created, integration with Linear."
94. [transcripts/2026-11-16-design-session-4.md](../transcripts/2026-11-16-design-session-4.md) line 196: "The original idea of the project was to cite everything based on claims in source code, but we have reduced the scope for the MVP to focus on distilling markdown docs files."
95. [transcripts/2026-11-16-design-session-4.md](../transcripts/2026-11-16-design-session-4.md) line 196: "refers to the desired end state after we iterate, not just the first commit of the first MVP we build."
96. [transcripts/2026-11-16-design-session-4.md](../transcripts/2026-11-16-design-session-4.md) line 94: "command line progress reports as well as a way of viewing the same progress updates through the web interface"
97. [transcripts/2026-11-16-design-session-4.md](../transcripts/2026-11-16-design-session-4.md) line 94: "It's really important that our UI is always clear and never appears to be frozen / blocking."
98. [transcripts/2026-11-16-design-session-4.md](../transcripts/2026-11-16-design-session-4.md) line 94: "show what is being processed, how many records exist, what the current iterator is, a progress bar or at a minimum a percentage complete at all times"
99. [transcripts/2026-11-16-design-session-4.md](../transcripts/2026-11-16-design-session-4.md) line 89: "Agreed, MVP has no login so localhost only"
100. [transcripts/2026-11-16-design-session-4.md](../transcripts/2026-11-16-design-session-4.md) line 90: "No javascript is fine, let's keep this minimal until we need it, no need for keyboard shortcuts in an MVP."
101. [transcripts/2026-11-16-design-session-4.md](../transcripts/2026-11-16-design-session-4.md) line 92: "Reqwest is approved, REST API is better than a SDK since it's more platform agnostic anyway"
102. [transcripts/2026-11-16-design-session-4.md](../transcripts/2026-11-16-design-session-4.md) line 98: "However things like pipeline checks, and the final data model are large projects in themselves to be iterated on later"
103. [transcripts/2026-11-16-design-session-4.md](../transcripts/2026-11-16-design-session-4.md) line 288: "Yes let's keep a minimal glossary of terms so we can keep everything consistent"
104. [transcripts/2026-11-16-design-session-4.md](../transcripts/2026-11-16-design-session-4.md) line 97: "Decisions made by AI should always be flagged as such so that they can be reviewed and we avoid the situation where an assumption becomes a documented technical requirement without human review"
105. [transcripts/2026-11-16-design-session-4.md](../transcripts/2026-11-16-design-session-4.md) line 200: "this was intended to be an illustrative example not a specific naming request"
106. [transcripts/2026-11-16-design-session-4.md](../transcripts/2026-11-16-design-session-4.md) line 287: "Atomic claims and compound claims sounds good"
107. [transcripts/2026-11-16-design-session-4.md](../transcripts/2026-11-16-design-session-4.md) line 99: "I noticed there were certain things that did not adhere to the writing guidelines, specifically referring to things as singular objects instead of categories."
108. [transcripts/2026-11-16-design-session-4.md](../transcripts/2026-11-16-design-session-4.md) line 99: "Even a short readme file with a few sentences could be condensed. AI writing loves adding fluff"
109. [transcripts/2026-11-16-design-session-4.md](../transcripts/2026-11-16-design-session-4.md) line 288: "I question if a separate glossary doc is actually necessary or if we can just use a small table"
110. [transcripts/2026-11-16-design-session-4.md](../transcripts/2026-11-16-design-session-4.md) line 202: "Let's be clear about the language we're using"
111. [transcripts/2026-11-16-design-session-4.md](../transcripts/2026-11-16-design-session-4.md) line 91: "let's use a similar setup to web-of-trust so we can keep things minimal without dependencies where it makes sense"
112. [transcripts/2026-11-16-design-session-4.md](../transcripts/2026-11-16-design-session-4.md) line 93: "I would prefer to have a config file instead of command line arguments."
113. [transcripts/2026-11-16-design-session-4.md](../transcripts/2026-11-16-design-session-4.md) line 204: "Config file structure confirmed, except for the previous note I made regarding the repos."
114. [transcripts/2026-11-16-design-session-4.md](../transcripts/2026-11-16-design-session-4.md) line 95: "Reviewer can be an option in the config file for now"
115. [transcripts/2026-11-16-design-session-4.md](../transcripts/2026-11-16-design-session-4.md) line 198: "The repos would be added to the project via the UI, not a config file."
116. [transcripts/2026-11-16-design-session-4.md](../transcripts/2026-11-16-design-session-4.md) line 286: "It's fine to keep repos in a specific directory defined via the config, I like that idea actually"
117. [transcripts/2026-11-16-design-session-4.md](../transcripts/2026-11-16-design-session-4.md) line 96: "Sounds good to keep the prompt tracked in git with a hash so we can tell the version and what records need to be updated"
118. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 79: "This makes sense to make reasons optional under acceptance but required for rejections"
119. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 80: "Agreed, we should not be building database tables for features we are not yet implementing"
120. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 83: "Agreed, we need code checks before we can determine how stale something is in the documentation"
121. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 84: "Good idea, reject all non-local requests for security"
122. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 85: "This is why we need API error handling"
123. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 87: "You should be writing tests for me to run, see other Wetfish projects as examples"
124. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 89: "Run execution can be synchronous to start but we should consider parallelizing tasks where it makes sense"
125. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 91: ""Declare source of truth" means referring to places in the source code or meeting transcripts that record a decision."
126. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 91: "This is out of scope for the first iteration of this MVP."
127. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 162: "Let's move on and propose changes in chat first before drafting everything."
128. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 213: "clap is fine as a dependency"
129. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 214: "Agreed no running hooks just clone the repo so we can look for docs files"
130. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 215: "Yes this is a necessity for conflict review and staleness review actully"
131. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 215: "we may be able to use git commands to determine how stale different parts of docs are if we determine a conflict between newer and older docs within the same project"
132. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 218: "use git as much as possible for the initial prototype"
133. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 219: "Eventually we'd want to support docs formatted as text files like readme.txt, html rich documentation, pdfs, etc. but for now md files exclusively are fine for a prototype"
134. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 222: "Sounds good let's just use the existing progress output from git"
135. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 224: "Let's avoid asking so many obvious questions like this and focus on building quickly so we can iterate"
136. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 360: "I've recently updated our AI development guidelines to ensure quick iterations and faster development workflow."
137. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 437: "I want to reprioritize the web interface first."
138. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 437: "This will allow me to view everything in the web interface step by step as we work through the rest of the code"
139. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 489: "Signed off on all decisions, let's build it"
140. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 552: "I've decided to create a new branch called `docs-mvp`"
141. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 731: "I want to make sure we are keeping track of API costs, estimates, tokens in, tokens out, etc."
142. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 784: "Decisions signed off, I particularly like #5 to have a limit."
143. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 784: "I think many of these options should be listed as config options"
144. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 784: "that is the sort of decision we'd want to do a trade study on to see the actual impacts, out of scope for this initial MVP."
145. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 874: "I think it should be an option in the config file, not the env."
146. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 1007: "let's avoid updating tests until the end"
147. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 1091: "Section split makes sense for a proof of concept, might need to be changed if we run into issues during testing"
148. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 1096: "This decision feels arbitrary and I can't really say for sure what I want without seeing the results. Let's iterate and decide"
149. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 1098: "Please update the handoff to get rid of all open questions, we want to be able to move quickly."
150. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 1136: "I do not like including intermediary files in long-lived storage."
151. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 1140: "we should begin thinking about how we can move away from including prompt transcripts and store this information (decisions, reasons for them, etc) within the data structure of the project itself."
152. [transcripts/2026-11-16-build-session-5.md](../transcripts/2026-11-16-build-session-5.md) line 1142: "by the end of this project we should be distilling the transcripts into only the important information so that we can get rid of them and clean up the repo."
