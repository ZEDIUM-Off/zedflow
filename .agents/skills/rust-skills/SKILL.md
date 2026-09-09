---
name: rust-skills
description: Load the global Rust engineering rules before writing, reviewing, debugging, optimizing, or refactoring Rust code.

skill_category: code-quality
skill_scope: project
skill_invocation: automatic
skill_projects:
- zedflow
skill_source: local:zedflow
skill_collection: code-quality
skill_group: rust/review
skill_kind: reference
skill_status: conflict
tags:
- skill/domain/code-quality
- skill/effect/local-write
- skill/language/rust
- skill/task/review
---

# Shared Rust rules

Read `/home/zedium/.agents/skills/rust-skills/SKILL.md` completely before Rust work. Then read every
linked file under `/home/zedium/.agents/skills/rust-skills/rules/` whose category is affected by the
change. Apply all relevant rules and report the validation commands run.

Repository instructions and executable lint configuration take precedence over a general rule when
they deliberately differ.
