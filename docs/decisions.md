# Decisions

The current decisions. Each section numbers its own decisions with its letter, most important first. The numbers change whenever a section is reordered, so they aren't permanent IDs. Each decision cites its sources by number. The references at the bottom give the file, line and exact quote. Rachel Fish reviews and commits every revision, and `git log -p docs/decisions.md` shows every earlier version.

## C: What Distillery is

- **C1.** A decision engine: a visual interface where people see programs as decision trees and the information relationships behind them. [1] [2]
- **C2.** Goal: distill projects to their essence, prove every fact, and build a network of decisions that forms the spec. [3]
- **C3.** Docs and transcripts become concise and cited to real decisions, so people can agree on the current state of tribal knowledge, and so slop and hallucinations don't get in. [4] [5]

## M: MVP

- **M1.** The MVP distills docs, as a proof of concept for the visual decision tree and requirements system. [6]
- **M2.** A Rust web server: the user pastes a git repo URL, the analysis runs, and the distilled docs appear with clickable citations to the source code. [7]
- **M3.** Input: the Markdown docs in a repo that also holds the code. [8] [9]
- **M4.** It finds stale information, redundancy, conflicting claims and needless detail, and has an interface for human review. [9] [10]
- **M5.** It checks that cited functions exist at the cited lines. Checking what the code actually does comes later. [11]
- **M6.** Output: the distilled docs, plus commands for the user to create a PR that updates the docs. [12]
- **M7.** What counts as one distilled fact is worked out by trying the process on real projects. [13]
- **M8.** One analysis of one snapshot. [14]

## P: How distillation works

- **P1.** Stages, each feeding the next: distill the docs; reviewers confirm facts against their own knowledge, which builds a requirements doc; check the code against the requirements; distill the code to its functional parts; improve implementations and data structures; finish with a security audit that proves everything works as intended and allows no unauthorized access. [15] [16] [17] [18] [19] [20]
- **P2.** Docs generated from the code are part of the distillation report, including for legacy projects with no docs. The same process runs on projects with and without docs, so docs generated from the code alone can be compared with what the docs say. [21] [22]
- **P3.** Every decision is confirmed by a person, who is recorded. Reviewers confirm facts and pick sides in conflicts, and explain why so others understand the reasoning. [23] [24]
- **P4.** Reviewers approve decisions for specific departments, sections of a project, or repos. [25]
- **P5.** Every analysis starts with the user declaring the source of truth. [26]
- **P6.** Each project's structure is discovered from its own evidence, never assumed from another project. [27]

## R: How decisions are recorded

- **R1.** This project's transcripts are its source of truth. [28]
- **R2.** The current state can be viewed as a diagram, such as a block diagram, fault tree or state matrix. [29]
- **R3.** Each section numbers its decisions with its own letter, ordered by priority. The numbers make decisions easy to reference, but they change and aren't IDs. Citations are auditable, and earlier decisions and changes stay on record. [30] [31] [32] [33]
- **R4.** Decisions cite numbered references to a file, line and exact quote. Transcripts, docs and source code are cited the same way. [34] [35]
- **R5.** Conventional Commits 1.0.0, in this project and in the PRs and issues Distillery creates for other projects. [36] [37]
- **R6.** Code changes and conversations are kept as concise as possible. [38]

## I: Implementation

- **I1.** Written in Rust. [39]
- **I2.** Supports multiple LLM platforms, with the Claude API as the benchmark. [40]
- **I3.** LLMs distill docs. Code analysis uses traditional tools wherever possible; the specific tools are still to be chosen. [41] [42]

## L: Later

- **L1.** Code analysis supports at minimum PHP, Rust, Python, JavaScript (including Node.js) and Go, starting with PHP. [43] [44] [45]
- **L2.** Running project test suites, coverage, mutation testing and performance analysis. [46]
- **L3.** Distill meeting transcripts in corporate settings into only the most important decisions: newest first, who said what, and who the responsible engineers are. [47] [48]
- **L4.** Run in CI: flag PRs that conflict with confirmed requirements, and only merge code that has cited documentation. [49]
- **L5.** Open PRs, create issues, and integrate with Linear. [50]

## References

1. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 263: "We are creating the decision engine interface. A visual interface so people can conceptualize programs as decision trees"
2. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 138: "We are building an information relationship visualization interface"
3. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 368: "building up a network of decisions that creates a spec of what is supposed to be"
4. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 265: "we want to make our docs & transcripts more concise, so it's easier for people to coordinate on the current state of tribal knowledge"
5. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 275: "making docs concise and cited to real decisions is the best way to prevent this kind of slop and hallucinations"
6. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 259: "D14 - MVP distills docs. Proof of concept for the visual decision tree & requirements specification system."
7. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 295: "Rust webserver, starts up, user pastes git repo into the web interface, analysis runs, docs generated, displayed in web interface with linked clickable citations that bring you to the source code"
8. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 289: "Markdown within a repo is fine for docs processing for now"
9. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 293: "Our MVP should simply accept a git repo with code & docs inside and then distill the docs in it to look for stale information, redundancy, conflicting claims, and an interface for human review."
10. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 275: "many AI generated documents are very long with needless details."
11. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 291: "For an MVP I think we should just check that the specific functions exist at the cited lines, but in production we need to do full code analysis and implementation"
12. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 261: "D16 - Both. Display the distilled docs and display the commands for a user to create a PR to update the docs"
13. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 290: "I think this should come from us analyzing a project and figuring this out by trying the process"
14. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 240: "But for right now, a single analysis run with a report."
15. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 170: "Once we have a concise view of "this is what the docs say" a reviewer can compare the distilled documentation against their own working knowledge."
16. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 170: "As details are confirmed during review, the system builds out a comprehensive requirements doc."
17. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 170: "Then these requirements feed back into the program and allow us to do another iteration: checking what the code code does against what it is supposed to do."
18. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 170: "Then, once we have reviewed that, we are able to distill the code to only the functional parts."
19. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 170: "are these implementations ideal? are there more efficient ways of doing this? are there better data structures to accomplish this goal?"
20. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 170: "we can do a final security audit iteration where we prove that everything works as intended and does not allow for unauthorized access"
21. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 273: "N1 - Docs generation should be a part of the distillation report, and we need to work in docs generation for projects which have no docs already, like old legacy codebases."
22. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 253: "We can use the same techniques on a project with documentation and without, and compare what we generate in a clean implementation (reading the code, no docs) versus reading what the docs say."
23. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 268: "Every decision should be confirmed, and it should say by who."
24. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 292: "a reviewer has to confirm specific facts & decisions and pick sides for conflicts, ideally explaining why so other team members can understand the reasoning behind decisions and scope changes"
25. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 268: "each person could be reviewing and approving decisions for specific departments, sections of the project, specific repos, etc."
26. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 136: "The user says what the source of truth is. When we start analyzing a new project we always need to know: what is the source of truth?"
27. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 366: "Copying the structure of this single repo will not help you analyze the structure of future repos"
28. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 269: "Transcripts are the source of truth for how we are creating this project"
29. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 285: "We need to have the current source of truth, something people can look at, like a block diagram, a fault tree, a state matrix."
30. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 285: "The references and citations it's built on should be auditable, with a record of all previous decisions and changes. I think the "decisions" list should be mutable, with decisions ranked by priority"
31. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 285: "doesn't pollute people's mind with random "decision ID numbers" that inevitably become invalidated"
32. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 361: "it's fine to use a numbered list for the decisions, so that it's easier to reference them, but we should recognize that the numbers are mutable and not set in stone, they are ordered by priority"
33. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 398: "I think each section should have its own prefix"
34. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 412: "provide htis conversation as a transcript in the distillery project and then mention citations referencing the transcript."
35. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 412: "In this way, a transcript, a docs file, or lines of source code can all be used interchangeably"
36. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 466: "include the use of Conventional Commits, https://www.conventionalcommits.org/en/v1.0.0/"
37. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 468: "I want to ensure that we are following these best practices as we build this project and as we create PRs & issues for other projects."
38. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 309: "I want us to keep our code changes and dialog exchanges as concise as possible"
39. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 124: "but I want this project itself to be written in Rust"
40. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 128: "for now I want to develop this in an architecture where we can use other platforms, but for now just using the Claude API as a benchmark."
41. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 265: "docs distillation makes sense to use LLMs, but we have to start specifying what types of tools and the specific implementation we'll be using for code analysis."
42. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 134: "we actually avoid using LLMs and use traditional code analysis tools as much as possible"
43. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 124: "We need to be as language agnostic as possible, I want to support projects written at a minimum: PHP, Rust, Python, JavaScript (& Node.js), Go."
44. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 124: "We can start with PHP as a starting language to process"
45. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 267: "D5, D6 - Agreed, this is only important for code analysis, which we can defer for now."
46. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 170: "This should be a long term goal, not an immediate need."
47. [transcripts/2026-11-11-design-session-2.md](../transcripts/2026-11-11-design-session-2.md) line 269: "ultimately what we will be distilling in corporate settings that have many transcribed meetings and we need to extract only the most important decisions from them"
48. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 127: "Meeting transcripts, ordered by recency, specifically who said what."
49. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 238: "We can automatically flag things like where someone submits a PR with code that conflicts against the confirmed requirements for the project, and only allow new code to be merged when it has cited documentation."
50. [transcripts/2026-11-11-design-session.md](../transcripts/2026-11-11-design-session.md) line 129: "Ideal end state: PRs against the repo, issues created, integration with Linear."
