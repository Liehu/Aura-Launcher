# Agent Task Template

```yaml
id: LAUNCHER-XXX
objective: ""
context: ""
scope:
  include: []
  exclude: []
constraints: []
api_changes: false
architecture_change: false
acceptance:
  - ""
tests:
  - ""
performance:
  required: false
  baseline: ""
rollback: ""
```

## Agent Plan

### Inspect

- relevant files:
- relevant tests:
- relevant ADRs:

### Plan

- design:
- risks:
- touched files:

### Implementation

Describe the smallest implementation path.

### Validation

```bash
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

### Completion Report

```text
Summary:
Changed files:
Tests:
Benchmark:
Memory impact:
Architecture impact:
Risks:
```
