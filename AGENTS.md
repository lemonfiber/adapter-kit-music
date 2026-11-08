# AGENTS.md — adapter-kit-music

> **Start at the roadmap and board on [lemonfiber.app](https://lemonfiber.app),
> rendered from the report of where every unreleased version stands. Then the
> rules** every repository shares:
> [working in the repositories](https://github.com/lemonfiber/spec/blob/main/50-governance/working-in-the-repositories.md)
> and [the rules for agents](https://github.com/lemonfiber/spec/blob/main/50-governance/ai-contributors.md).
> This file holds only what is true of this repository.

## What this repo is

`music.curate` as the adapters in front of the stack's music curators answer it,
Lidarr first, as one Rust library crate. Each adapter takes it as a git dependency
at an exact `rev`. It serves no contract of its own, builds no image and has no
binary ([spec](https://github.com/lemonfiber/spec/blob/main/30-repos/adapter-kit-music.md)).

## The rules you cannot break

- **Only what the music curators' adapters share** (`REPO-R104`). What one
  upstream alone answers stays in that upstream's adapter, and the Servarr client
  is taken from `adapter-kit-servarr`, never copied.
- **No adapter's release, digest, settings, recorder or recordings** (`REPO-R106`).
- **The core's crates move with the train.** Every `lemonfiber-*` crate in
  `Cargo.toml` names one core commit, and `adapter-kit-servarr` and every adapter
  taking this kit name the same one (`REPO-R105`).
- **No `unwrap`, `expect`, `panic` or indexing** outside tests. The lints in
  `Cargo.toml` are the core's.
- **No credential in a refusal.** What an upstream says is withheld before it is
  carried into a failure, and only its own words are taken from it.
- **Every line is covered** (`REPO-R107`), over a fake HTTP transport.

## Checks

```
just ci
```
