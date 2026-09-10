# Working on GitHub Actions

Read [../../docs/development.md](../../docs/development.md) for release conventions.
These workflows follow `codesoda/discuss-cli` conventions adapted to `xcli`; do
not edit that other repository as part of changes here.

## CI checklist

- [ ] Preserve stable macOS/Ubuntu checks and Rust **1.88** MSRV coverage.
- [ ] Preserve warnings-denied formatting → Clippy → build → test → rustdoc
      validation, committed lockfile use, and no live/ignored credential tests.
- [ ] Aislop reads `.aislop/config.yml`: gate **95**, otherwise default settings.
      Do not lower the gate or silently drop a failing check.
- [ ] Reusable CI checks the exact supplied commit, not a moving branch head.
- [ ] Checkout tokens remain read-only/nonpersistent where possible. Restrict
      `contents: write` to the release publishing job. Never expose tokens in logs
      or execute untrusted PR code with privileged `pull_request_target` access.

## Release checklist

- [ ] Validate tag syntax before using input; resolve an existing exact tag and
      verify it matches the Cargo package version. Never substitute main.
- [ ] Require one nonempty matching `## [X.Y.Z]` or `## [vX.Y.Z]` changelog section.
      An Unreleased-only changelog must not permit publication.
- [ ] Run the required CI gate against the resolved immutable commit before builds
      and publication. Recheck that the tag still points at the tested commit.
- [ ] Keep Apple Silicon/Intel macOS target names, native runners and locked
      release build/test steps aligned. Do not imply other-platform auth support.
- [ ] Package `xcli-<tag>-<target>.tar.gz` with the executable at the archive root;
      publish `checksums-sha256.txt` and the validated changelog notes. Missing
      assets/notes must fail rather than produce a partial release.
- [ ] After authorized publication, the dedicated `verify-installed` jobs download
      the published installer/binaries and explicitly run credential-free public
      smoke checks. This is separate from normal offline CI and never invokes
      browser/Keychain tests. Preserve the evidence artifact for each architecture.
- [ ] Do not create a tag, trigger publishing or release a binary as a smoke test.
      Publication requires an explicit maintainer request.

Run `actionlint .github/workflows/*.yml` from the repository root. When changing
release validation, test malformed tags, absent/duplicate/empty changelog
sections and version mismatches using synthetic inputs. Static/local validation
is not proof that a GitHub-hosted workflow ran successfully.
