---
title: Stabilization (v0.2.1 Patch Release)
summary: Weeks 1-2 tasks — enable CI/CD, fix bugs, documentation, release prep
tags: [stabilization, release, ci-cd, testing]
---

# Stabilization (Weeks 1-2)

> Prepare v0.2.1 patch release with bug fixes, CI/CD, and process improvements.

---

## Checklist

### Week 1: Fixes & CI/CD

- [ ] **Bugs #1-3 fixed** (JWT, TLS, Backpressure)
  - [ ] All unit tests passing
  - [ ] Code review completed
  - [ ] Commits pushed to `fix/*` branches

- [ ] **CI/CD enabled**
  - [ ] Move `ipfrs_source/.github/workflows/ci.yml.disabled` → `ci.yml`
  - [ ] Test locally: `cargo build -p ipfrs --all-features`
  - [ ] Test locally: `cargo nextest run --all-features`
  - [ ] GitHub Actions runs on push
  - [ ] No regressions in test suite

- [ ] **Documentation standards added**
  - [ ] Create `SECURITY.md` (see template below)
  - [ ] Create `CONTRIBUTING.md` (link to this RoadMap)
  - [ ] Create `CODE_OF_CONDUCT.md` (standard Rust community)

- [ ] **GitHub settings configured**
  - [ ] Branch protection: require 1 review before merge
  - [ ] Require status checks (CI/CD)
  - [ ] Allow auto-delete head branches on merge

### Week 2: Remaining Bugs & Release

- [ ] **Bugs #4-6 fixed** (GC, FedAvg, Arrow)
  - [ ] All unit tests passing
  - [ ] Benchmarks show no regression
  - [ ] Code review completed

- [ ] **Version bump & changelog**
  - [ ] Update `ipfrs_source/Cargo.toml`: `version = "0.2.1"`
  - [ ] Update `ipfrs_source/CHANGELOG.md`:
    ```markdown
    ## [0.2.1] - 2026-07-03 "Stability Patch"
    
    ### Fixed
    - JWT: Replace MD5 with HS256 HMAC (#1)
    - TLS: Implement real certificate generation, not stub (#2)
    - Backpressure: Release semaphore permits on window decrease (#3)
    - GC: Apply min_age parameter (#4)
    - FedAvg: Fix timeout when min_peers > 0 (#5)
    - Arrow: Optimize serialization (remove extra copies) (#6)
    
    ### Added
    - Security policy (SECURITY.md)
    - Contributing guide (CONTRIBUTING.md)
    - Code of Conduct
    - GitHub Actions CI/CD enabled
    ```

- [ ] **Tag & release**
  - [ ] Create annotated tag: `git tag -a v0.2.1 -m "Stability patch release"`
  - [ ] Push tag: `git push origin v0.2.1`
  - [ ] GitHub Releases: Create from tag (auto-populate from CHANGELOG.md)

- [ ] **Publish artifacts (optional)**
  - [ ] `cargo publish` to crates.io (if upstream allows)
  - [ ] npm package update for `@cool-japan/ipfrs-node` (if published)
  - [ ] PyPI update for `ipfrs` (if published)

---

## File Templates

### SECURITY.md

```markdown
# Security Policy

## Reporting Vulnerabilities

**DO NOT open a public GitHub issue for security vulnerabilities.**

Email your report to: **[your-email-here]** with subject `[SECURITY] IPFRS: <vulnerability title>`

Include:
- Description of the vulnerability
- Affected versions (e.g., 0.2.0, 0.2.1)
- Steps to reproduce
- Potential impact
- Suggested fix (optional)

## Supported Versions

| Version | Status | Security Updates |
|---------|--------|------------------|
| 0.2.1 | Current | ✅ Yes |
| 0.2.0 | Previous | ✅ Yes (backports) |
| 0.1.x | EOL | ❌ No |

## Known Limitations & Fixes

### v0.2.0 (Fixed in v0.2.1)
- ⚠️ JWT authentication used MD5 instead of HS256 → **FIXED**
- ⚠️ TLS certificate generator returned stub cert → **FIXED**
- ⚠️ Backpressure window decrease didn't release permits → **FIXED**
- ⚠️ GC `min_age` parameter was ignored → **FIXED**
- ⚠️ FedAvg always timed out with `min_peers > 0` → **FIXED**

## Security Best Practices for Users

When running IPFRS in production:

1. **Always use HTTPS/TLS** (enable with rustls)
2. **Rotate peer identities** regularly (PeerIdentityManager)
3. **Enable encryption at rest** for sensitive data (ChaCha20-Poly1305 or AES-256-GCM)
4. **Limit peer connections** via ConnectionManager
5. **Monitor GossipSub mesh health** for eclipse attacks
6. **Use circuit breaker** for failed peer connections
7. **Enable Prometheus metrics** for observability

## Disclosure Timeline

### For Critical Vulnerabilities
- Day 0: Report received
- Day 1-2: Assessment & fix development
- Day 3-5: Testing & review
- Day 6-7: v0.2.x patch release
- Day 8: Public disclosure

## Acknowledgments

Security fixes are attributed in releases. Thank you for helping keep IPFRS secure!
```

### CONTRIBUTING.md

```markdown
# Contributing to IPFRS

Thank you for your interest in contributing! This project follows Apache 2.0 licensing and welcomes community contributions.

## Getting Started

1. **Read the RoadMap:** [../RoadMap/README.md](../RoadMap/README.md)
2. **Understand the architecture:** See `Wiki_Arch_Claude/` for 15 detailed articles
3. **Set up local development:**
   ```bash
   cd ipfrs_source
   cargo build --all-features
   cargo test --all-features
   ```

## Contribution Types

### Bug Fixes (Easiest)
- Pick from [01-Critical-Bugs.md](../RoadMap/01-Critical-Bugs.md)
- Follow the PR template in each bug description
- Include unit test
- Link the GitHub issue in your PR

### Documentation
- Improve `Wiki_Arch_Claude/` articles
- Write tutorials in `ipfrs_source/book/src/`
- Fix typos / examples

### Features
- Check [04-Features.md](../RoadMap/04-Features.md) for approved ideas
- Discuss large features in GitHub Discussions first
- Include tests & documentation

### Testing & Benchmarks
- Improve test coverage (run `cargo tarpaulin`)
- Add benchmarks (`cargo bench`)
- Profile performance with `cargo flamegraph`

## PR Process

1. Create a branch: `git checkout -b fix/your-issue-name`
2. Make changes + tests + documentation
3. Commit with clear message: `git commit -m "fix: description"`
4. Push: `git push -u origin fix/your-issue-name`
5. Open PR on GitHub with:
   - Clear title
   - Reference to issue (`Fixes #123`)
   - Description of changes
   - Checklist (test, doc, lint)

### PR Checklist

```markdown
## Description
[Describe what you're fixing/adding]

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Documentation
- [ ] Performance improvement

## Testing
- [ ] Unit tests added/updated
- [ ] Existing tests pass: `cargo test --all-features`
- [ ] No clippy warnings: `cargo clippy -- -D warnings`

## Documentation
- [ ] Code comments added (if non-obvious)
- [ ] `Wiki_Arch_Claude/` articles updated
- [ ] `CHANGELOG.md` entry added

## Related Issue
Fixes #<issue-number>
```

## Code Style

- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` before committing
- Address all `cargo clippy` warnings
- Comments should explain **why**, not **what** (code is self-documenting)

## Architecture Notes

IPFRS uses **Domain-Driven Design** with 5 bounded contexts:
- **Storage** (Sled, blocks, GC)
- **Network** (libp2p, DHT, peers)
- **Semantic** (HNSW, vector search)
- **TensorLogic** (inference engines, Datalog)
- **Transport** (Bitswap, sessions)

See `Wiki_Arch_Claude/03-BoundedContexts.md` for details.

## Upstream Contributions

This is a fork of [cool-japan/ipfrs](https://github.com/cool-japan/ipfrs). **Bug fixes and non-breaking features should be upstreamed!**

See [05-Upstream-Contribution.md](../RoadMap/05-Upstream-Contribution.md) for how to contribute upstream.

## Code Review

Maintainers will review:
- [ ] Correctness & safety
- [ ] Test coverage
- [ ] Performance impact
- [ ] Documentation quality
- [ ] Backward compatibility

## Community

- **Questions?** Open a GitHub Discussion
- **Found a security issue?** See `SECURITY.md`
- **Want to discuss ideas?** GitHub Discussions or email

---

**Thank you for contributing to IPFRS! 🚀**
```

### CODE_OF_CONDUCT.md

```markdown
# Rust Community Code of Conduct

## Our Pledge

In the interest of fostering an open and welcoming environment, we as contributors and maintainers pledge to making participation in our project and our community a harassment-free experience for everyone, regardless of age, body size, disability, ethnicity, gender identity and expression, level of experience, nationality, personal appearance, race, religion, or sexual identity and orientation.

## Our Standards

Examples of behavior that contributes to creating a positive environment include:

- Using welcoming and inclusive language
- Being respectful of differing opinions, viewpoints, and experiences
- Gracefully accepting constructive criticism
- Focusing on what is best for the community
- Showing empathy towards other community members

Examples of unacceptable behavior by participants include:

- The use of sexualized language or imagery and unwelcome sexual attention or advances
- Trolling, insulting/derogatory comments, and personal or political attacks
- Public or private harassment
- Publishing others' private information without explicit permission
- Other conduct which could reasonably be considered inappropriate

## Reporting & Enforcement

Instances of abusive, harassing, or otherwise unacceptable behavior may be reported by contacting the project team privately. All reports will be reviewed and investigated.

Project maintainers who do not follow or enforce this Code of Conduct in good faith may face temporary or permanent repercussions.

## Scope

This Code of Conduct applies both within project spaces and in public spaces when an individual is representing the project or its community.

---

**This Code of Conduct is adapted from the [Contributor Covenant](http://contributor-covenant.org/).**
```

---

## Testing Checklist

### Before Release

```bash
# Full test suite
cargo test --all-features --all --verbose

# Clippy (no warnings)
cargo clippy --all-features --all-targets -- -D warnings

# Format check
cargo fmt --check

# Security audit
cargo audit

# Benchmarks (no regressions)
cargo bench --all-features

# Code coverage (target: >80%)
cargo tarpaulin --all-features --timeout 300 --out Html

# Documentation build
cd ipfrs_source/book && mdbook build

# Try building without features
cargo build --no-default-features
```

### CI/CD Verification

```bash
# Simulate GitHub Actions locally
act push  # Requires act: https://github.com/nektos/act
```

---

## Release Notes Template

```markdown
# IPFRS v0.2.1 - Stability Patch

**Release Date:** July 3, 2026

## Overview

v0.2.1 is a patch release that fixes 6 critical and high-priority bugs found during 
code analysis, enables CI/CD, and establishes security & contribution policies.

### Download

- **Source:** [GitHub Release](https://github.com/yunusovt983-art/ipfrs-ai/releases/tag/v0.2.1)
- **Crates.io:** `cargo add ipfrs@0.2.1`
- **npm:** `npm install @cool-japan/ipfrs-node@0.2.1`

## Security Fixes

| Priority | Issue | Fix |
|----------|-------|-----|
| 🔴 CRITICAL | JWT used MD5 | Now uses HS256 HMAC |
| 🔴 CRITICAL | TLS stub cert | Now generates real self-signed certs |
| 🟠 HIGH | Backpressure | Permits now released on window decrease |
| 🟠 HIGH | GC min_age | Parameter now respected |
| 🟡 MEDIUM | FedAvg timeout | Fixed async collection logic |
| 🟡 MEDIUM | Arrow copies | Memory usage optimized |

## What's New

- Security policy (`SECURITY.md`)
- Contributing guide (`CONTRIBUTING.md`)
- Code of Conduct
- GitHub Actions CI/CD enabled
- Complete test coverage maintained

## Upgrade Guide

No breaking changes. Simply update your dependency:

```toml
[dependencies]
ipfrs = "0.2.1"
```

## Checklist for v0.3.0

- [ ] Release v0.2.1 stable
- [ ] Continue 0.3.0 development (Intelligence Release)
- [ ] Community feedback & issues
```

---

## Timeline

| Date | Task | Status |
|------|------|--------|
| Day 1-2 | Fix bugs #1-2 | ⬜ |
| Day 3-4 | Fix bug #3 | ⬜ |
| Day 5 | Enable CI/CD, documentation | ⬜ |
| Day 6-7 | Fix bugs #4-6 | ⬜ |
| Day 8 | Release v0.2.1 | ⬜ |

**See also:** [TIMELINE.md](TIMELINE.md) for detailed week-by-week plan.
