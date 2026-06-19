---
title: Upstream Contribution (Ongoing)
summary: How to contribute fixes and features back to cool-japan/ipfrs
tags: [upstream, contribution, open-source]
---

# Upstream Contribution (Ongoing)

> This fork (`yunusovt983-art/ipfrs-ai`) is based on Apache 2.0 codebase.  
> **All bug fixes and non-breaking features should be upstreamed to the original project.**

---

## Philosophy

This is **not a competitive fork**. The goal is to:

1. **Stabilize** IPFRS v0.2.1 (fix critical bugs)
2. **Document** architecture comprehensively
3. **Extend** with useful features
4. **Contribute upstream** (PR → original repo)

**Both repositories benefit.** The original project gets stability & features; your fork gets recognition & easier maintenance.

---

## What to Upstream

### ✅ Upstream (always)

- **Bug fixes** (all 6 critical bugs)
- **Security patches**
- **Performance improvements**
- **Test improvements** (more coverage)
- **Documentation** (architecture, tutorials)
- **CI/CD fixes**
- **Dependency updates** (security alerts)

### ⚠️ Discuss First (might be controversial)

- **New features** (discuss in original repo issue first)
- **API changes** (breaking or not)
- **Architecture refactors**
- **New dependencies**

### ❌ Keep Local (fork-specific)

- **Your extended documentation** (Wiki_Arch_Claude stays here)
- **Your deployment guides** (specific to your infrastructure)
- **Experimental features** (not yet stable)
- **Fork-branding** (GitHub name, links)

---

## Process: Contributing a Bug Fix

Let's say you want to fix Bug #1 (JWT MD5 → HS256).

### Step 1: Create Issue in Original Repo

```bash
# Go to https://github.com/cool-japan/ipfrs/issues/new

Title: "Security: JWT uses MD5 instead of HS256"

Body:
---
## Summary
JWT tokens are encoded with MD5 (insecure) instead of HS256 (HMAC-SHA256).

## Impact
- Tokens can be forged
- Authentication can be bypassed
- Severity: **CRITICAL**

## Location
- File: `crates/ipfrs-interface/src/auth.rs:449`
- Method: `encode_token()`

## Suggested Fix
Replace with HS256 HMAC encoding.

## Reference
Found during security analysis of v0.2.0 codebase.
See: https://github.com/yunusovt983-art/ipfrs-ai/issues/XX (your fork)
---
```

### Step 2: Create Feature Branch in Original Repo

```bash
# Clone original repo
git clone https://github.com/cool-japan/ipfrs.git
cd ipfrs

# Create feature branch
git checkout -b fix/jwt-hs256-auth

# ... make changes to crates/ipfrs-interface/src/auth.rs ...

# Test locally
cargo test -p ipfrs-interface --all-features
cargo clippy -p ipfrs-interface -- -D warnings
```

### Step 3: Open PR in Original Repo

```bash
git push -u origin fix/jwt-hs256-auth

# Then open PR on GitHub:
# https://github.com/cool-japan/ipfrs/compare/main...fix/jwt-hs256-auth
```

**PR Template:**

```markdown
## Title
fix: Replace JWT MD5 with HS256 HMAC

## Description
Fixes #123 (the issue you created)

### Changes
- [ ] Replace `jsonwebtoken` encoding to use HS256
- [ ] Update unit test to verify algorithm
- [ ] Add CHANGELOG entry

### Testing
- [x] `cargo test` passes
- [x] `cargo clippy` clean
- [x] No breaking changes
- [x] Backward compatible (API unchanged)

### Checklist
- [x] Tests added/updated
- [x] Documentation updated
- [x] CHANGELOG.md entry added
- [x] No unsafe code added

### Reference
Found during code analysis in https://github.com/yunusovt983-art/ipfrs-ai
```

### Step 4: Respond to Review

- [ ] Maintainers will review
- [ ] Make requested changes
- [ ] Respond to comments professionally
- [ ] Push follow-up commits (don't force-push)

### Step 5: Merge & Celebrate

- [ ] PR gets merged into `main`
- [ ] Your commit is now in upstream
- [ ] Your fork cherry-picks the fix (or stays in sync)

---

## Process: Contributing a Feature

More complex. Let's say you implement the S3 Gateway (Feature #2).

### Step 1: Discuss in Issue

```markdown
# RFC: S3-Compatible REST Gateway

## Proposal
Add S3-compatible REST API layer to IPFRS for enterprise adoption.

## Motivation
- Drop-in replacement for cloud storage (boto3, AWS CLI, etc.)
- Enterprise customers expect S3-compatible APIs
- Differentiates IPFRS from IPFS

## Design
[Describe API, endpoints, implementation plan]

## Effort Estimate
~30 hours development

## Questions for Maintainers
- Is this aligned with v0.3.0 roadmap?
- Should it be a separate crate or built into ipfrs-interface?
- Any concerns about maintenance burden?

## Next Steps
- [ ] Get feedback from maintainers
- [ ] Create detailed design doc if approved
- [ ] Start implementation in feature branch
```

### Step 2: Get Approval

Wait for maintainer feedback before starting major work.

**Good signs:**
- ✅ "This would be valuable"
- ✅ "Let's start with MVP"
- ✅ "Already planned for v0.3.0"

**Red flags:**
- ❌ "Out of scope"
- ❌ "We're going a different direction"
- ❌ No response after 1 week (might be busy)

### Step 3: Implementation (Same as Bug Fix)

- Create feature branch
- Implement with tests
- Open PR when ready
- Respond to review feedback

### Step 4: Iteration

Features might need multiple iterations before merge.

---

## Tips for Successful Upstream PRs

### Before Opening

- [ ] **Search existing issues** (don't duplicate)
- [ ] **Run full test suite**: `cargo test --all-features`
- [ ] **Run clippy**: `cargo clippy --all -- -D warnings`
- [ ] **Format code**: `cargo fmt`
- [ ] **Write tests** (mandatory for fixes, expected for features)
- [ ] **Update CHANGELOG.md** (describe your change)
- [ ] **Check git history** (meaningful commit messages)

### PR Description

- [ ] **Clear title** (fix/feat: ...)
- [ ] **Reference issue** (Fixes #123)
- [ ] **Explain why** (not just what)
- [ ] **Testing** (how to verify)
- [ ] **Breaking changes** (if any)
- [ ] **Checklist** (tests, docs, clippy)

### Communication

- [ ] **Respond promptly** to feedback
- [ ] **Ask clarifying questions** (don't assume)
- [ ] **Acknowledge concerns** ("good point, I'll...")
- [ ] **Don't be defensive** (constructive criticism helps)
- [ ] **Celebrate approval** (thank reviewers)

### Large PRs

If your PR is >500 lines:

- [ ] **Create draft PR** first (shows intent)
- [ ] **Get early feedback** before finishing
- [ ] **Break into smaller commits** (easier to review)
- [ ] **Document rationale** (why each part exists)

---

## Dealing with Rejection

Sometimes PRs don't get merged. That's OK.

**Common reasons:**
- "Not aligned with project goals"
- "Too much maintenance burden"
- "Already working on similar feature"
- "Architectural concerns"

**What to do:**

1. **Ask why** ("Thanks for the feedback. Can you explain the concern?")
2. **Understand the constraint** (maybe there's a good reason)
3. **Offer alternatives** ("What if we did X instead?")
4. **Keep it local** (your fork can still have it)
5. **Move on** (no hard feelings)

---

## Tracking Contributions

### In Your Fork

```markdown
## Upstream Status

| Contribution | Status | PR Link | Date |
|--------------|--------|---------|------|
| Bug #1: JWT MD5 | ✅ Merged | cool-japan/ipfrs#123 | 2026-07-01 |
| Bug #2: TLS stub | ✅ Merged | cool-japan/ipfrs#124 | 2026-07-03 |
| Bug #3: Backpressure | 🔄 Review | cool-japan/ipfrs#125 | 2026-07-05 |
| S3 Gateway | 💭 Proposed | cool-japan/ipfrs#150 | 2026-07-20 |
```

### In README.md

```markdown
## Contributing Upstream

This fork has contributed 4+ upstream PRs:
- [PR #123](https://github.com/cool-japan/ipfrs/pull/123) — Fix JWT security
- [PR #124](https://github.com/cool-japan/ipfrs/pull/124) — Implement TLS certs
- [PR #125](https://github.com/cool-japan/ipfrs/pull/125) — Fix backpressure

**Current:** 6/6 critical bugs identified; 3/6 merged upstream.

**Next:** S3 gateway feature (proposed, awaiting feedback).

See [RoadMap/](RoadMap/) for detailed tracking.
```

---

## Questions for Maintainers

Good questions to ask when proposing upstream work:

- "Is this aligned with your roadmap?"
- "Should I start with an issue or jump to PR?"
- "Do you have bandwidth to review in the next 2 weeks?"
- "Any architectural concerns with this approach?"
- "Would you prefer one big PR or multiple smaller ones?"
- "What's the review process? (CI, code review, etc.)"

---

## Syncing Your Fork with Upstream

As original repo evolves, keep your fork in sync:

```bash
# Add upstream remote
git remote add upstream https://github.com/cool-japan/ipfrs.git

# Fetch latest
git fetch upstream

# Rebase your local changes on top of upstream main
git rebase upstream/main

# Or merge (keeps history)
git merge upstream/main

# Push to your fork
git push origin main
```

---

## License & Attribution

- Your fork: Apache 2.0 (same as original)
- All contributions: attributed to you (in commit history)
- Original authors: always credited (see LICENSE & NOTICE)

---

## Example: Real Contribution Timeline

**Date** | **Action** | **Status**
---|---|---
2026-06-19 | You identify 6 critical bugs | 📝 Analysis
2026-06-26 | Fix bugs #1-3 locally, test | ✅ Local
2026-07-01 | Open upstream issue #123 (JWT) | 💭 Discussion
2026-07-02 | Upstream maintainer comments | 🔄 Feedback
2026-07-03 | You open PR with fix | ✅ PR Opened
2026-07-05 | Maintainer reviews, requests changes | 🔄 Review
2026-07-06 | You push follow-up commit | ✅ Updated
2026-07-08 | Maintainer approves, merges | ✅ Merged!
2026-07-10 | Original repo releases v0.2.1 with your fix | 🎉 Released

---

**Remember:** Open source is a marathon, not a sprint. Quality contributions compound over time.

**Next:** [TIMELINE.md](TIMELINE.md) — Week-by-week execution roadmap.
