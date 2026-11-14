//! Removing what Lidarr holds with its files, excluding nothing from a later import: an
//! album alone, with its artist where Lidarr wants nothing else of theirs and the house
//! holds none of it, or an artist with every album.

use lemonfiber_adapter_kit::endpoint::form_encoded;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::Failure;

use super::{Curator, ALBUMS, ARTISTS, THE_ALBUMS, THE_ARTISTS};
use crate::wire::{Artist, Held};

/// What a removal tells Lidarr: take the files with it and exclude nothing from a later
/// import.
const WITH_ITS_FILES: &str = "deleteFiles=true&addImportListExclusion=false";

/// The lengths of the groups of hexadecimal digits a MusicBrainz id is written in, joined
/// by hyphens.
const MUSICBRAINZ_ID: [usize; 5] = [8, 4, 4, 4, 12];

/// Said where Lidarr holds no album by the reference.
const NO_SUCH_ALBUM: &str = "it holds no such album";

/// Said where Lidarr holds no artist by the reference.
const NO_SUCH_ARTIST: &str = "it holds no such artist";

/// Remove the album `reference` names alone with its files, and its artist with it where
/// Lidarr wants no other album of theirs and the house holds none.
pub(super) async fn album(curator: &Curator, reference: &str) -> Result<(), Failure> {
    if !musicbrainz_id(reference) {
        return Err(curator.refused(NO_SUCH_ALBUM));
    }
    let query = form_encoded(&[("foreignAlbumId", reference)]);
    let found: Vec<Held> = curator
        .servarr
        .read(&format!("{ALBUMS}?{query}"), THE_ALBUMS)
        .await?;
    let one = only(found, |one| {
        one.foreign_album_id.as_deref() == Some(reference)
    })
    .ok_or_else(|| curator.refused(NO_SUCH_ALBUM))?;
    let alone = match one.artist_id {
        Some(by) => {
            let theirs: Vec<Held> = curator
                .servarr
                .read(&format!("{ALBUMS}?artistId={by}"), THE_ALBUMS)
                .await?;
            (!theirs
                .iter()
                .any(|other| other.id != one.id && other.kept()))
            .then_some(by)
        }
        None => None,
    };
    removed(curator, ALBUMS, one.id).await?;
    match alone {
        Some(by) => removed(curator, ARTISTS, by).await,
        None => Ok(()),
    }
}

/// Remove the artist `reference` names with every album and its files.
pub(super) async fn artist(curator: &Curator, reference: &str) -> Result<(), Failure> {
    if !musicbrainz_id(reference) {
        return Err(curator.refused(NO_SUCH_ARTIST));
    }
    let query = form_encoded(&[("mbId", reference)]);
    let found: Vec<Artist> = curator
        .servarr
        .read(&format!("{ARTISTS}?{query}"), THE_ARTISTS)
        .await?;
    let id = only(found, |one| one.foreign_artist_id == reference)
        .and_then(|one| one.id)
        .ok_or_else(|| curator.refused(NO_SUCH_ARTIST))?;
    removed(curator, ARTISTS, id).await
}

/// The one of `found`, where it holds exactly one and `named` holds of it.
fn only<T>(found: Vec<T>, named: impl Fn(&T) -> bool) -> Option<T> {
    let mut found = found.into_iter();
    match (found.next(), found.next()) {
        (Some(one), None) if named(&one) => Some(one),
        _ => None,
    }
}

/// The one at `path` with Lidarr's `id` removed with its files.
async fn removed(curator: &Curator, path: &str, id: i64) -> Result<(), Failure> {
    curator
        .servarr
        .done(
            Method::Delete,
            &format!("{path}/{id}?{WITH_ITS_FILES}"),
            None,
        )
        .await
}

/// Whether `reference` is written as a MusicBrainz id, as every reference the kit gives
/// is.
fn musicbrainz_id(reference: &str) -> bool {
    let groups: Vec<&str> = reference.split('-').collect();
    groups.len() == MUSICBRAINZ_ID.len()
        && groups.iter().zip(MUSICBRAINZ_ID).all(|(group, length)| {
            group.len() == length && group.bytes().all(|digit| digit.is_ascii_hexdigit())
        })
}

#[cfg(test)]
mod tests;
