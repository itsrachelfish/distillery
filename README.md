# distillery

A decision engine: distill a project's docs and code into key facts, cited by source, confirmed by humans.

v1 distills Markdown docs in git repos. Find stale, redundant, and conflicting claims. Link facts to source code.

- [docs/decisions.md](docs/decisions.md): current decisions, cited
- [docs/mvp-plan.md](docs/mvp-plan.md): MVP development plan
- [transcripts/](transcripts/): prompts & transcripts

## Setup

```bash
cp distillery.example.toml distillery.toml
cargo run
```

Prints settings in use and creates database. Tests: `cargo test`.
