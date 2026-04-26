---
name: comparison_tables_for_query_changes
description: When presenting query improvements for a language, show a before/after table comparing different patterns
type: feedback
---

When presenting tree-sitter query improvements for a language, show a before/after comparison table covering the different language patterns (e.g. methods, decorators, fields). This makes it easy to see what changed at a glance.

**Why:** User explicitly requested this format — it's clearer than prose descriptions of what changed.

**How to apply:** After validating query changes against a real project, produce a table with columns: Pattern | Before | After. Include both changed and unchanged patterns to show nothing regressed.
