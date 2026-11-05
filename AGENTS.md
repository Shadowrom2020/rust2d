# AGENTS.md

Guidelines for anyone (people and coding agents) working on rust2d: a Rust 2D top-down RPG
engine (`crates/engine`), the game runner (`crates/game`) and the editor (`crates/editor`).

## Branching concept

```
feature/a            o---o                      feature/b   o---o
                    /     \                                /     \
develop     ---o---o-------o---------o--------o-----------o-------o---
                                      \      /  (release merged back)
release/0.0.4                          o----o
                                             \
master      ---o------------------------------o--- tag v0.0.4
```

| Branch | Created from | Merged into | Purpose |
|---|---|---|---|
| `master` | – | – | The main branch. Only ever updated by merging a release branch. Every merge is a release and gets a tag `vX.Y.Z`. |
| `develop` | `master` (once) | – | Integration branch: the consolidated state of all finished features. Must always build and pass the checks below. |
| `feature/<name>` | `develop` | `develop` | One feature or fix per branch, e.g. `feature/quest-journal`, `feature/fix-script-tab`. |
| `release/<version>` | `develop` | `master`, then `develop` | Created on demand when one or more features on `develop` should become a release, e.g. `release/0.0.4`. |

Note: this repository's main branch is called `master` (not `main`).

### Rules

1. **Never commit directly to `master` or `develop`.** All work happens on a feature branch.
   `develop` only receives merges from feature branches (and release branches being merged
   back). `master` only receives merges from release branches.
2. **Features** start from an up-to-date `develop` and go back into `develop` when they are
   done and checked. Keep a feature branch focused on one topic; delete it after the merge.
3. **Releases** are cut on demand from `develop` once it contains the features to ship. A
   release branch only gets release work: the version bump in the workspace `Cargo.toml`,
   README / release notes, and small fixes found while preparing the release. No new features.
4. Finishing a release: merge `release/<version>` into `master` with `--no-ff`, tag the merge
   commit `v<version>` on `master`, merge the release branch back into `develop` (so the version
   bump and fixes are not lost), then delete the release branch.
5. Versions follow `MAJOR.MINOR.PATCH` (currently `0.x.y`); the version lives in
   `[workspace.package] version` in the root `Cargo.toml`.
6. Push branches and tags only when asked to; publish a branch with `git push -u origin <branch>`.
7. Before starting work, check which branch is checked out (`git branch --show-current`) and
   that the working tree is clean. If the current branch is `master` or `develop`, create a
   feature branch first.

### Commands

```sh
# Start a feature
git checkout develop && git pull --ff-only
git checkout -b feature/<name>

# Finish a feature (after the checks below pass)
git checkout develop && git pull --ff-only
git merge --no-ff feature/<name>
git push origin develop
git branch -d feature/<name>                 # and: git push origin --delete feature/<name>

# Cut a release from the consolidated develop branch
git checkout develop && git pull --ff-only
git checkout -b release/0.0.4
#   bump [workspace.package] version in Cargo.toml, update README, commit "rust2d 0.0.4"

# Finish the release
git checkout master && git pull --ff-only
git merge --no-ff release/0.0.4
git tag v0.0.4
git checkout develop && git merge --no-ff release/0.0.4
git push origin master develop v0.0.4
git branch -d release/0.0.4
```

## Checks before merging

Run from the repository root; all must pass before a feature or release branch is merged:

```sh
cargo build --workspace                      # also rebuilds rust2d-game for editor play-tests
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- If `crates/engine/examples/gen_example` changed, regenerate the example project and the
  editor's starter asset pack with `cargo run --example gen_example` and commit the results.
  Never delete or commit `example_project/saves/` (it is ignored; it may hold the user's saves).
- Build the game in release mode (`cargo build --release --workspace`) before testing
  *File → Export game*.

## Commit messages

- A short summary line, then a blank line and bullet points for the notable changes.
- Release commits on release branches: `rust2d <version>`.

## Project notes

- Map and definition files must stay loadable by older and newer versions: append new enum
  variants at the end, and give new fields `#[serde(default)]`.
- Tiles are always 32 px (`project::TILE_SIZE`).
- Features are verified end to end with scripted runs: `RUST2D_AUTOPLAY` for the game and
  `RUST2D_EDITOR_SCRIPT` for the editor (see `crates/game/src/main.rs` and
  `crates/editor/src/main.rs` for the commands), plus screenshots. Run destructive tests on a
  copy of `example_project`, and set `RUST2D_EDITOR_CONFIG` / `RUST2D_LIBRARY` to scratch folders
  so the user's recent-projects list and asset library are not touched.
