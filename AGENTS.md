# Working conventions

Work in the shared checkout and coordinate file ownership between agents. Commit completed changes locally as you work. Use `master` as the main branch. Do not force-push or rewrite others’ commits unless explicitly instructed.

Do not add comments to code, tests, configuration, or scripts. Preserve executable shebangs and tool-required directives. Documentation is limited to material the user explicitly requested: the brief root README, monorepo area READMEs with build instructions and hardware status, this file, and the changelog.

Changelog tracking begins after milestone 4 implementation. Do not backfill milestones 1–4. For each subsequent user-visible change, add a concise entry under `## [Unreleased]` in `CHANGELOG.md`, using `Added`, `Changed`, or `Fixed` as appropriate. Internal refactoring and test-only changes do not require entries. The Release workflow promotes Unreleased notes into the tagged version; do not manually remove those notes while implementing changes.

Board-specific Rust code belongs in `toucan/` or `paintbrush/`; shared platform code belongs in `firmware/`, portable support crates in `shared/`, the Rust component runtime in `host/`, and all Wavelet sources, WIT, build tooling, and generated components in `clients/`. The shared manifests explicitly include board-owned modules and tests.

Run focused tests and relevant cross-builds before committing. Keep checked-in component artifacts consistent with their Wavelet sources using `scripts/build-components.sh`. Normal builds and release artifacts must use the same canonical keymap. Support only the two three-board keyboard setups; the target catalog and release directory must contain exactly six production images. Do not reintroduce standalone modes, demo firmware, or diagnostic/reset images without an explicit request. Distinguish code and cross-build verification from real hardware validation. Keep release versions synchronized through `scripts/release-version.py`; release publication runs through the manual GitHub Actions workflow.
