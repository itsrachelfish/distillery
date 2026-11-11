# Decisions

Each decision cites its sources by number. The references at the bottom give the file, line number, and exact quote. A transcript, a doc, or source code can all be cited this way.

Confirmed by Rachel Fish on 2026-11-11 [1], except D20 and D21, which she stated directly in the same session.

| ID | Decision | Sources |
|----|----------|---------|
| D1 | Distillery is an information relationship visualization interface. | [2] |
| D2 | Goal: distill projects to their essence, prove every fact, and build a network of decisions that forms the spec. | [3] |
| D3 | Each project's structure is discovered from its own evidence, never assumed from another project. | [4] |
| D4 | Distillery is written in Rust. | [5] |
| D5 | As language-agnostic as possible; supports at minimum PHP, Rust, Python, JavaScript (including Node.js), and Go. | [6] |
| D6 | PHP is the first language to process. | [7] |
| D7 | The architecture supports multiple LLM platforms; the Claude API is the benchmark for now. | [8] |
| D8 | Use traditional code analysis tools over LLMs wherever possible. | [9] |
| D9 | Every analysis starts with the user declaring the source of truth. | [10] |
| D10 | Every decision is confirmed by a person and records who confirmed it. | [11] |
| D11 | Reviewers approve decisions for specific departments, sections of a project, or repos. | [12] |
| D12 | Reviewers agree or disagree with decisions and record why. | [13] |
| D13 | The MVP includes the review interface and the decision graph. | [14] |
| D14 | The MVP analysis checks docs against code, with verified citations. | [15] |
| D15 | The MVP runs a single analysis of one snapshot. | [16] |
| D16 | The MVP outputs a report of changes that could be made as PRs and issues that could be created. | [17] |
| D17 | Later: run in CI, flag PRs that conflict with confirmed requirements, and only allow merging code that has cited documentation. | [18] |
| D18 | Later: open PRs, create issues, and integrate with Linear. | [19] |
| D19 | Later: order meeting transcripts by recency and track who said what and who the responsible engineers are; compare those decisions to the docs, then compare the code to the approved docs and requirements. | [20] |
| D20 | Decisions cite sources with numbered references to a file, line number, and exact quote. Transcripts, docs, and source code are cited the same way. This design session is kept in the repo as a transcript. | [21] [22] |
| D21 | Follow Conventional Commits 1.0.0 in this project and in the PRs and issues Distillery creates for other projects. | [23] [24] |

## References

1. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 412: "The candidate decisions you've provided seem good"
2. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 138: "We are building an information relationship visualization interface"
3. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 368: "building up a network of decisions that creates a spec of what is supposed to be"
4. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 366: "Copying the structure of this single repo will not help you analyze the structure of future repos"
5. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 124: "but I want this project itself to be written in Rust"
6. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 124: "We need to be as language agnostic as possible, I want to support projects written at a minimum: PHP, Rust, Python, JavaScript (& Node.js), Go."
7. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 124: "We can start with PHP as a starting language to process"
8. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 128: "for now I want to develop this in an architecture where we can use other platforms, but for now just using the Claude API as a benchmark."
9. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 134: "we actually avoid using LLMs and use traditional code analysis tools as much as possible"
10. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 136: "The user says what the source of truth is. When we start analyzing a new project we always need to know: what is the source of truth?"
11. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 268: "Every decision should be confirmed, and it should say by who."
12. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 268: "each person could be reviewing and approving decisions for specific departments, sections of the project, specific repos, etc."
13. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 132: "There needs to be an interface where people can actually review the citations and agree or disagree with the decisions and record why."
14. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 268: "The review interface and building out the tree of decisions and source of truth are the most important pieces we need to be building in this project, and therefore the MVP has to cover these pieces."
15. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 124: "Great idea on the 1 operation MVP (checking docs against code, with verified citations)."
16. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 240: "But for right now, a single analysis run with a report."
17. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 129: "Initial MVP: a report of changes that could be made in PRs and a report of issues that could be created."
18. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 238: "We can automatically flag things like where someone submits a PR with code that conflicts against the confirmed requirements for the project, and only allow new code to be merged when it has cited documentation."
19. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 129: "Ideal end state: PRs against the repo, issues created, integration with Linear."
20. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 127: "Meeting transcripts, ordered by recency, specifically who said what. Who are the responsible engineers and what have they said in meetings about what is being built? Then, compare that against all documentation and identify gaps & contradictions. Then, compare all source code to the approved documentation & requirements and identify gaps in the code & things that need to be updated"
21. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 412: "provide htis conversation as a transcript in the distillery project and then mention citations referencing the transcript. ([1], [2], [3], etc.) with a reference of line numbers and quotes at the bottom."
22. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 412: "In this way, a transcript, a docs file, or lines of source code can all be used interchangeably"
23. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 466: "include the use of Conventional Commits, https://www.conventionalcommits.org/en/v1.0.0/"
24. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 468: "I want to ensure that we are following these best practices as we build this project and as we create PRs & issues for other projects."
