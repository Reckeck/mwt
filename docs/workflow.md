# How to work: branches, pre-releases, releases

How code gets from a task to the user and what CI does at each step.
How CI itself is built (workflows, images, caches, secrets) is in [ci.md](ci.md).

## Branches

| Branch | Created from | Merged into | What CI does |
|---|---|---|---|
| `feature/<task>` | from `development` | PR into `development` | checks only |
| `hotfix/<task>` | from `main` | PR into `main` | checks only |
| `development` | — | PR into `main` (release) | checks → builds for all platforms → **pre-release** `X.Y.Z-N` |
| `main` | — | — | checks → builds → **release** `X.Y.Z` → PR back into `development` |

- Into `development` and `main` only through a PR, with no direct pushes and
  no force-push (protected branches).
- Builds start only when **all** checks are green; a red check
  cancels the `release.yml` run before publication.
- Tags are not created by hand: the release job creates them together with the GitHub Release.

## Versions

The version is `X.Y.Z` in `crates/app/tauri.conf.json`, `[workspace.package]` in
`Cargo.toml`, `Cargo.lock` and `<release>` in
`packaging/flatpak/*.metainfo.xml`. It is not changed by hand: `mise run bump`
does it (see "Release") together with the `CHANGELOG.md` section.

- **Pre-release** from `development`: `X.Y.Z-N`, where `N` is the last
  published `vX.Y.Z-N` of the same `X.Y.Z` plus one, otherwise 1 (digits only,
  as MSI requires). If `X.Y.Z` is already released and no bump
  has been done yet, the pre-release becomes `X.Y.(Z+1)-N` by itself. For pacman this is `X.Y.ZpreN`,
  which sorts before the release `X.Y.Z` and will upgrade to it.
- **Release** from `main`: exactly `X.Y.Z`, tag `vX.Y.Z`. The `version` job
  stops the release if `vX.Y.Z` is already released or `CHANGELOG.md`
  has no `## [X.Y.Z]` section, which means `mise run bump` was forgotten on `development`.

## CHANGELOG

`CHANGELOG.md` is built from commits: they follow Conventional Commits
(`feat`, `fix`, `perf`, … — see AGENTS.md), and `git-cliff` (`cliff.toml`)
sorts them into the groups "Added", "Fixed", "Performance"…

- `mise run changelog` updates the commit list in `## [Unreleased]`
  (between `<!-- commits:start/end -->`). Above it you can write by hand
  the main points for the user; that text is preserved.
- `mise run bump` picks the next version from the commits since the last release (`feat` →
  minor, everything else → patch; `mise run bump patch|minor|major|X.Y.Z` sets it
  explicitly), writes the version into all the files, and `[Unreleased]` becomes
  `## [X.Y.Z] — date`. 1.0 and any major only explicitly.
- CI writes the pre-release notes in GitHub Releases itself: the commits since the last release.

Both commands work in the main (colocated) copy: they need the git history,
which a jj side workspace does not have.
- The Android `versionCode` is the run number: it always grows, so any
  next build installs over the previous one.

## A task: from start to pre-release

```bash
jj git fetch
jj new development@origin                 # new work on top of development
# … edits, `mise run check` …
jj describe -m "feat(…): …"               # contextual commits, see AGENTS.md
jj bookmark create feature/<task> -r @
jj git push -b feature/<task>             # CI: checks only (PR)
gh pr create --base development --head feature/<task> --fill
```

PR green and reviewed → merge → `release.yml` on `development` builds everything and
publishes a pre-release (GitHub Releases, `vX.Y.Z-N`, the files are release attachments).
The last 10 pre-releases are kept, older ones are deleted
(`cargo xtask prune-prereleases`, needs `GH_TOKEN`).

## Release

The release steps are the commands `cargo xtask publish`, `prune-prereleases`,
`backmerge`, `publish-aur` (token `GH_TOKEN`, see [ci.md](ci.md#release-through-xtask-github)).
They are called by `.github/workflows/release.yml` (graph, secrets, manual run:
[ci.md](ci.md#releaseyml)).

1. In `development` (the main copy): `mise run bump` for the version and the CHANGELOG
   section; optionally write the main points in words above the commit list;
   `mise run check`; commit `chore(release): vX.Y.Z`, push, PR into
   `development` (or directly as part of the release PR).
2. PR `development` → `main`, merge.
3. `release.yml` on `main`: checks → builds → Release `vX.Y.Z` with files and
   notes from CHANGELOG (Linux, Windows, Android, Flatpak, pacman; macOS is
   paused for now). AUR via the `aur` job (needs the secret `AUR_SSH_PRIVATE_KEY`). The bundles can
   also be checked locally before the merge: `mise run build:all` (`docs/ci.md`).
4. `backmerge` opens a PR `main` → `development` (`cargo xtask backmerge`, needs `GH_TOKEN`);
   merge it. There is no need to raise the version right away: pre-releases until the next
   bump go out as `X.Y.(Z+1)-N` on their own.

## Hotfix

```bash
jj new main@origin
# … the fix (a `fix: …` commit), then `mise run bump patch` …
jj bookmark create hotfix/<task> -r @ && jj git push -b hotfix/<task>
gh pr create --base main --head hotfix/<task> --fill
```

Merge → release `vX.Y.(Z+1)` → `backmerge` brings the fix into
`development`.

## Watching CI

```bash
gh pr checks                   # checks of the current branch's PR
gh run list --branch <branch>  # workflow runs of a branch
gh run view <id> --log-failed  # log of the failed steps
gh run watch <id>              # wait for the run to finish
gh pr list                     # open PRs
```

The workflow files themselves are checked with `mise x actionlint@1.7.12 -- actionlint`.

## One-time GitHub setup

1. **The `development` branch** from `main`, made the default branch
   (Settings → General → Default branch): PRs go there by default, and
   the Actions caches are visible to branches created from it.
2. **Branch protection** (Settings → Branches): `main` and `development` — only
   through a PR, the required check `ci-ok`, no force-push.
3. **Automatic branch deletion** (Settings → General → "Automatically delete head
   branches"): `cache-cleanup.yml` then cleans the caches of merged branches.
4. **Secrets and permissions** (Settings → Secrets and variables → Actions):
   the Android signing keys, the updater key `TAURI_SIGNING_*`, `AUR_SSH_PRIVATE_KEY` — see [ci.md](ci.md#secrets-settings--secrets-and-variables--actions);
   turn on "Allow GitHub Actions to create and approve pull requests" for
   `backmerge`.
