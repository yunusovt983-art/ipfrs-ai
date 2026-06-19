---
title: IPFRS Development RoadMap
summary: Strategic roadmap for yunusovt983-art/ipfrs-ai fork — stabilization, community building, features
tags: [roadmap, strategy, timeline, milestones]
updated: 2026-06-19
---

# IPFRS Development RoadMap

> Strategic plan for `yunusovt983-art/ipfrs-ai` fork based on Apache 2.0 codebase analysis.

## Quick Overview

```
📊 Project Status: v0.2.0 "Network Release" (Production Ready)
🎯 Fork Goal: Stabilize, document, extend, contribute upstream
👥 Team: You + community PRs
⏱️  Timeline: 5-8 weeks intensive, then ongoing maintenance
```

## Navigation

1. **[01-Critical-Bugs.md](01-Critical-Bugs.md)** — Fix 6 security/correctness issues (Weeks 1-2)
2. **[02-Stabilization.md](02-Stabilization.md)** — v0.2.1 patch release, enable CI/CD (Weeks 1-2)
3. **[03-Community-Docs.md](03-Community-Docs.md)** — Finish book, tutorials, policies (Weeks 3-4)
4. **[04-Features.md](04-Features.md)** — Choose 1-2 major features for v0.3.0 support (Weeks 5+)
5. **[05-Upstream-Contribution.md](05-Upstream-Contribution.md)** — How to push upstream (ongoing)
6. **[TIMELINE.md](TIMELINE.md)** — Week-by-week execution plan

---

## Key Metrics

| Metric | Target | Status |
|--------|--------|--------|
| Critical bugs fixed | 6 / 6 | ⬜ Not started |
| v0.2.1 release | ✅ | ⬜ Not started |
| CI/CD enabled | ✅ | ⬜ Not started |
| Book chapters | 8 / 8 | ⬜ In progress |
| GitHub Discussions | ✅ | ⬜ Not started |
| Security policy | ✅ SECURITY.md | ⬜ Not started |
| Community contrib guide | ✅ CONTRIBUTING.md | ⬜ Not started |
| Upstream PRs | 3+ | ⬜ Not started |

---

## 🚀 Quick Start (Day 1)

```bash
cd /Volumes/Kingston/cool-japan

# 1. Create tracking issues
gh issue create -t "Bug: JWT uses MD5 instead of HS256" -b "See RoadMap/01-Critical-Bugs.md"
gh issue create -t "Chore: Enable CI/CD pipeline" -b "See RoadMap/02-Stabilization.md"
gh issue create -t "Docs: Finish mdbook chapters" -b "See RoadMap/03-Community-Docs.md"

# 2. Start bug fixes (Week 1)
git checkout -b fix/jwt-md5-auth
# Fix interface/src/auth.rs:449
# ... commit ...
git push -u origin fix/jwt-md5-auth
gh pr create --title "fix: Replace JWT MD5 with HS256" --body "Closes #1"

# 3. Track progress
# Update TIMELINE.md weekly
```

---

## License & Attribution

- **Upstream:** [cool-japan/ipfrs](https://github.com/cool-japan/ipfrs) — Apache 2.0
- **Fork:** [yunusovt983-art/ipfrs-ai](https://github.com/yunusovt983-art/ipfrs-ai) — Apache 2.0
- **Authors:** TensorLogic Architect (original) + Community (this fork)

**Note:** All PRs to this fork should be upstreamed to original project where possible.

---

## Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-06-19 | Start with bug fixes, not new features | Unblocks v0.2.1 release, builds community trust |
| 2026-06-19 | Fork under Apache 2.0 | Maintain license compatibility, enable upstream PRs |
| 2026-06-19 | Archive other Wiki variants | Keep Wiki_Arch_Claude as source of truth (gitignore others) |
| 2026-06-19 | Prioritize CI/CD | Prevents regression, enables safe refactoring |

---

**Last Updated:** 2026-06-19  
**Next Review:** 2026-06-26 (Week 1 checkpoint)
