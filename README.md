# adapter-kit-music

**`music.curate` as lemonfiber's music curators answer it.**
A household member asks a music curator for an album or a whole artist over
`music.curate`. This crate answers it once, for every adapter in front of a music
curator: the lookup of an album or an artist by words, a page at a time; one of them
by its reference; the request that monitors an album alone or an artist with every
album and those that come later, then searches where asked; and the root folders and
profiles a request is made with. Lidarr is the first curator it
answers for, in its v1 API, through the client `adapter-kit-servarr` holds.

## How an adapter takes it

`adapter_kit_music::curator::Curator::new(servarr)` answers `MusicCurating` through
the Servarr client the adapter already holds for Lidarr.

```toml
[dependencies]
adapter-kit-music = { git = "https://github.com/lemonfiber/adapter-kit-music", rev = "<commit>" }
```

The adapter names the same core commit for its `lemonfiber-*` crates as this
crate's `Cargo.toml` does. Nothing here is published to a registry.

## Checks

The crate is held to its format, its lints, its tests, full line coverage and its
dependency policy. `just ci` runs them.

## Licence

The code in this repository is under the licence in `LICENSE`.
