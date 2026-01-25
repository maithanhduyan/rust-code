# Proposed: [Feature/Architecture Name]

> **Status:** 📋 Proposed | 🔄 In Review | ✅ Approved | ❌ Rejected | 🚧 In Progress | ✔️ Completed
> **Date:** YYYY-MM-DD
> **Author:** [Name]
> **Priority:** 🔴 Critical | 🟠 High | 🟡 Medium | 🟢 Low
> **Complexity:** High | Medium | Low
> **Estimated Effort:** X days/weeks
> **Reviewed by:** [Names or "Pending"]

---

## 📋 Executive Summary

<!-- 2-3 sentences explaining WHAT this proposal is about and WHY it matters -->

[Brief description of the feature/change and its business value]

---

## 🎯 Goals & Non-Goals

### Goals
<!-- What this proposal WILL achieve -->
- [ ] Goal 1
- [ ] Goal 2
- [ ] Goal 3

### Non-Goals
<!-- What this proposal will NOT address (scope boundaries) -->
- ❌ Non-goal 1
- ❌ Non-goal 2

---

## 📊 Current State vs Proposed State

| Aspect | Current | Proposed |
|--------|---------|----------|
| [Aspect 1] | [Current behavior] | [New behavior] |
| [Aspect 2] | [Current behavior] | [New behavior] |

---

## 🏗️ Architecture / Design

<!-- Include diagrams, code snippets, data models -->

### System Diagram

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│ Component A │────►│ Component B │────►│ Component C │
└─────────────┘     └─────────────┘     └─────────────┘
```

### Data Models

```rust
// Example struct
pub struct Example {
    pub field1: String,
    pub field2: i64,
}
```

### API / Interface

```json
// Example request/response
{
  "input": "...",
  "output": "..."
}
```

---

## 📁 File Structure

<!-- What files will be added/modified -->

```
src/
├── new_module/
│   ├── mod.rs          ← NEW
│   └── feature.rs      ← NEW
└── existing/
    └── file.rs         ← MODIFY
```

---

## 🔧 Implementation Plan

### Phase 1: [Phase Name]

| # | Task | Output | Est. Time | Status |
|---|------|--------|-----------|--------|
| 1.1 | Task description | Deliverable | X hours | ⬜ |
| 1.2 | Task description | Deliverable | X hours | ⬜ |

### Phase 2: [Phase Name]

| # | Task | Output | Est. Time | Status |
|---|------|--------|-----------|--------|
| 2.1 | Task description | Deliverable | X hours | ⬜ |
| 2.2 | Task description | Deliverable | X hours | ⬜ |

---

## ⚠️ Risks & Mitigations

| Risk | Impact | Likelihood | Mitigation |
|------|--------|------------|------------|
| [Risk 1] | High/Medium/Low | High/Medium/Low | [How to prevent/handle] |
| [Risk 2] | High/Medium/Low | High/Medium/Low | [How to prevent/handle] |

---

## 📊 Success Metrics

<!-- How do we know this is successful? -->

| Metric | Target | Measurement Method |
|--------|--------|--------------------|
| [Metric 1] | [Value] | [How to measure] |
| [Metric 2] | [Value] | [How to measure] |

---

## 🔄 Alternatives Considered

### Option A: [Alternative Name]
- **Pros:** ...
- **Cons:** ...
- **Why rejected:** ...

### Option B: [Alternative Name]
- **Pros:** ...
- **Cons:** ...
- **Why rejected:** ...

---

## 🔗 Dependencies

<!-- External dependencies, blockers, or related proposals -->

- **Depends on:** [Other proposal/feature]
- **Blocks:** [What this proposal blocks]
- **Related:** [Related proposals/docs]

---

## ❓ Open Questions

<!-- Unresolved questions that need discussion -->

1. [Question 1]?
2. [Question 2]?

---

## 📚 References

- [Reference 1](url)
- [Reference 2](url)
- Related docs: [IDEA.md](../IDEA.md)

---

## 📝 Decision Log

<!-- Track decisions made during review -->

| Date | Decision | Rationale | Decided by |
|------|----------|-----------|------------|
| YYYY-MM-DD | [Decision] | [Why] | [Who] |

---

## ✅ Approval Checklist

- [ ] Technical review completed
- [ ] Security review (if applicable)
- [ ] Performance impact assessed
- [ ] Documentation updated
- [ ] Tests planned
- [ ] Rollback plan defined

---

*Last updated: YYYY-MM-DD*
