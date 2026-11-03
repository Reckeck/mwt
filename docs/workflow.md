# How to work: branches, pre-releases, releases

How code gets from a task to the user and what CI does at each step.
How CI itself is built (workflows, images, caches, secrets) is in [ci.md](ci.md).

## Branches

| Branch | Created from | Merged into | What CI does |
|---|---|---|---|
| `feature/<task>` | from `development` | PR into `development` | checks only |
| `hotfix/<task>` | from `main` | PR into `main` | checks only |
| `release/vX.Y.Z` | from `development` (`release-start`) | PR into `main`, squash, auto-merge | checks only |
| `backmerge/vX.Y.Z` | release tip or `development` (`backmerge --merge`) | PR into `development`, merge commit, auto-merge | checks only |
| `development` | — | — | checks → builds for all platforms → **pre-release** `X.Y.Z-N` |
| `main` | — | — | checks → builds → **release** `X.Y.Z` → back-merge into `development` |

- Into `development` and `main` only through a PR, with no direct pushes and
  no force-push (protected branches).
- Builds start only when **all** checks are green; a red check
  cancels the `release.yml` run before publication.
- Tags are not created by hand: the release job creates `vX.Y.Z` together with the GitHub Release,
  `release-start` creates `release-base/vX.Y.Z` (see "Release").

## Versions

The version is `X.Y.Z` in `crates/app/tauri.conf.json`, `[workspace.package]` in
`Cargo.toml`, `Cargo.lock` and `<release>` in
`packaging/flatpak/*.metainfo.xml`. It is not changed by hand: `release-start` does it (see
"Release"; `mise run bump` is the same code for local use and hotfixes) together with the
`CHANGELOG.md` section.

- **Pre-release** from `development`: `X.Y.Z-N`, where `N` is the last
  published `vX.Y.Z-N` of the same `X.Y.Z` plus one, otherwise 1 (digits only,
  as MSI requires). If `X.Y.Z` is already released and no bump
  has been done yet, the pre-release becomes `X.Y.(Z+1)-N` by itself. For pacman this is `X.Y.ZpreN`,
  which sorts before the release `X.Y.Z` and will upgrade to it.
- **Release** from `main`: exactly `X.Y.Z`, tag `vX.Y.Z`. The `version` job
  stops the release if `vX.Y.Z` is already released or `CHANGELOG.md`
  has no `## [X.Y.Z]` section (a release made by `release-start` always has both).

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

"Since the last release" is counted from the tag `release-base/vX.Y.Z` (the tip of the last release
branch), not from `vX.Y.Z`: the release is squashed into `main`, so `vX.Y.Z` sits on a commit that is
not an ancestor of `development`'s own commits, while `release-base/vX.Y.Z` is (the back-merge brings it).
`cliff.toml` (`tag_pattern`) and `crates/xtask/src/release.rs` (`boundary`) use it; on a hotfix branch
from `main` the nearest `vX.Y.Z` is the boundary. Pre-release notes (`publish --pre`) start at the
nearest of the pre-release tag and `release-base/*`. Before the first release there is no boundary and
the whole history is listed.

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

A release is started by hand and goes through pull requests only; there is no direct push to
`development` or `main`. The commands are hidden xtask commands (`release-start`, `backmerge --merge`,
`publish`, `prune-prereleases`, `publish-aur`), called by the workflows
(`release-start.yml`, `release.yml`; graph, secrets, manual run: [ci.md](ci.md#releaseyml)).

1. **Start**: Actions → `release-start` → Run workflow on `development` (no inputs;
   `gh workflow run release-start.yml` does the same). `cargo xtask release-start`:
   - the version is `auto`: git-cliff over the commits since `release-base/*`, never lower than the
     files say; the very first release ships the version of the files;
   - branch `release/vX.Y.Z` from `development`, the bump and the CHANGELOG section (the same code as
     `mise run bump`), the commit `chore(release): vX.Y.Z`, the tag `release-base/vX.Y.Z` on its tip;
   - if `main` does not exist yet (the first run), it is created from `development` first;
     the ruleset requires `ci-ok` also on creation (`do_not_enforce_on_create: false`), so this push
     can be refused: then nothing else has happened; create `main` once as an admin or relax that
     setting and run again;
   - an open release PR of the same version is closed, its branch and tag deleted, and the release is
     redone from the current `development`; an open release PR of another version stops the run;
   - PR `release/vX.Y.Z` → `main` with `gh pr merge --auto --squash`.
   To write the main points in words, add them to `## [Unreleased]` on `development` before the start.
   `cargo xtask release-start --dry-run` prints the steps and writes nothing (safe locally); a real
   run is refused outside GitHub Actions.
2. **Checks**: the PR runs `ci.yml`. Red: the PR stays open, nothing is released; fix `development`
   and start again (the old PR is replaced). Green: GitHub squashes the PR into `main`: one commit
   per release.
3. **Publish**: `release.yml` on `main`: checks → builds → Release `vX.Y.Z` (the tag is on the squash
   commit) with files and notes from CHANGELOG (Linux, Windows, Android, Flatpak, pacman; macOS is
   paused for now). AUR via the `aur` job. Creating `main` is a push too, but `release.yml` ignores
   a push that creates a branch.
4. **Back-merge**: `release.yml` runs `cargo xtask backmerge --merge --version X.Y.Z`: the branch
   `backmerge/vX.Y.Z` is the release tip (`release-base/vX.Y.Z`) plus `git merge -s ours main`, a PR
   into `development` with `gh pr merge --auto --merge`. It is done only after checking that the
   tree of `main` equals the tree of the release tip (the squash is exactly the release); a plain
   merge of `main` would conflict falsely, because the squash repeats what `development` already has.
   After the merge `development` has both the squash commit (so `vX.Y.Z`) and `release-base/vX.Y.Z`.
   There is no need to raise the version: pre-releases go out as `X.Y.(Z+1)-N` on their own.
   If `main` has other content than the release tip, the job fails and says so: merge by hand.

The bundles can be checked locally before a release: `mise run build:all` ([ci.md](ci.md)).
`mise run bump` stays for local use (a hotfix, or trying the next version); the release itself does
not need it.

## Hotfix

```bash
jj new main@origin
# … the fix (a `fix: …` commit), then `mise run bump patch` …
jj bookmark create hotfix/<task> -r @ && jj git push -b hotfix/<task>
gh pr create --base main --head hotfix/<task> --fill
```

The PR is merged into `main` by hand, with any merge method. `release.yml` publishes
`vX.Y.(Z+1)`, then `backmerge --merge` sees that there is no `release-base/vX.Y.(Z+1)` and makes
a plain merge of `main` into `development` (branch `backmerge/vX.Y.(Z+1)` from `development`, PR,
auto-merge); the tree check does not apply. A conflict leaves the PR `main` → `development`
open and fails the job with the reason in the Job Summary. The hotfix commits then appear once more in
the commit list of the next release (they are not behind its boundary); edit that list by hand if needed.

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
   `RELEASE_APP_ID` and `RELEASE_APP_PRIVATE_KEY` (the GitHub App of the release: contents, pull
   requests and workflows write; installed on the repository). It replaces `GITHUB_TOKEN` where a
   push or a PR has to start the checks.
5. **Merging** (Settings → General → Pull Requests): "Allow auto-merge", "Allow merge commits" and
   "Allow squash merging" on (`allow_auto_merge`, `allow_merge_commit`, `allow_squash_merge`).
   No ruleset bypass for the App: its PRs wait for `ci-ok` like any other. The ruleset must let the App
   create the branch `main` once (the first `release-start`).
