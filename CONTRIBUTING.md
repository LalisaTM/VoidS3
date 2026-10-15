# Contributing

This repository is a for-fun experiment in completely AI-written software. The current code has not been read, reviewed, or edited by a human. If a human reviews or changes implementation code, update the README provenance statement to describe that change truthfully.

Use a feature/fix branch and a PR, Conventional Commit-style messages, locked dependencies, and the checks in README.md. Keep file I/O in Rust, secrets in the OS keychain, and destructive actions explicit. Do not commit credentials or fabricated test/performance results. Do not describe a feature as complete without implementation and validation evidence.

The original brief is a target, not an implemented-feature list. Update docs/status.md and docs/validation.md when changing capabilities. Dependency bots and automated agents may open PRs; CI passing is not a security audit.
