//! Asking Lidarr for an album alone or an artist with every album and those that come
//! later: added where it holds none of it, monitored where it does, then searched for
//! where asked.

use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{Candidate, Failure, MusicAsk, MusicCurating as _};
use serde_json::{json, Map, Value};

use super::{
    Curator, ALBUMS, ALBUM_LOOKUP, ARTISTS, ARTIST_LOOKUP, BY_ID, THE_ALBUMS, THE_ARTISTS,
};
use crate::wire::{Album, Artist, Held};

/// Where Lidarr is told which albums it wants.
const ALBUM_MONITOR: &str = "/album/monitor";

/// Where Lidarr is told which artists it wants.
const ARTIST_EDITOR: &str = "/artist/editor";

/// Where Lidarr is told to search.
const COMMAND: &str = "/command";

/// What Lidarr's monitoring of an artist is called where it takes every album.
const EVERY_ALBUM: &str = "all";

/// What Lidarr's monitoring of an artist's new albums is called where it takes none.
const NO_ALBUM: &str = "none";

/// What Lidarr's monitoring of an artist is called where it takes only the albums named,
/// leaving the artist wanted.
const NAMED_ALBUMS: &str = "unknown";

/// Said where the catalogue holds no such reference.
const NOT_IN_THE_CATALOGUE: &str = "its catalogue holds no such reference";

/// Ask for the album `ask` names alone, searching for it where asked.
pub(super) async fn album(curator: &Curator, ask: &MusicAsk) -> Result<Candidate, Failure> {
    let found = resource(curator, ALBUM_LOOKUP, "foreignAlbumId", ask, THE_ALBUMS).await?;
    let read: Album = decoded(curator, found.clone(), THE_ALBUMS)?;
    let id = match read.id {
        Some(id) => {
            if let Some(by) = read.artist_id {
                let edited = json!({ "artistIds": [by], "monitored": true });
                curator
                    .servarr
                    .done(Method::Put, ARTIST_EDITOR, Some(edited.to_string()))
                    .await?;
            }
            monitored(curator, &[id]).await?;
            id
        }
        None => alone(curator, ask, found).await?,
    };
    if ask.search {
        commanded(curator, json!({ "name": "AlbumSearch", "albumIds": [id] })).await?;
    }
    standing(curator, ask).await
}

/// Lidarr's id for the album `found` once added alone under its artist, the artist
/// filed as `ask` says and wanted, taking none of its other albums or those that come
/// later.
async fn alone(
    curator: &Curator,
    ask: &MusicAsk,
    found: Map<String, Value>,
) -> Result<i64, Failure> {
    let mut body = found;
    body.insert("monitored".to_owned(), Value::Bool(true));
    body.insert(
        "addOptions".to_owned(),
        json!({ "searchForNewAlbum": false }),
    );
    let mut by = body
        .remove("artist")
        .and_then(|by| match by {
            Value::Object(by) => Some(by),
            _ => None,
        })
        .unwrap_or_default();
    filed(&mut by, ask, NO_ALBUM);
    by.insert(
        "addOptions".to_owned(),
        json!({
            "monitor": NAMED_ALBUMS,
            "albumsToMonitor": [ask.reference],
            "searchForMissingAlbums": false,
        }),
    );
    body.insert("artist".to_owned(), Value::Object(by));
    let added: Held = added(curator, ALBUMS, body, THE_ALBUMS).await?;
    Ok(added.id)
}

/// Ask for the artist `ask` names with every album and those that come later, searching
/// for them where asked.
pub(super) async fn artist(curator: &Curator, ask: &MusicAsk) -> Result<Candidate, Failure> {
    let found = resource(curator, ARTIST_LOOKUP, "foreignArtistId", ask, THE_ARTISTS).await?;
    let read: Artist = decoded(curator, found.clone(), THE_ARTISTS)?;
    if let Some(id) = read.id {
        let edited =
            json!({ "artistIds": [id], "monitored": true, "monitorNewItems": EVERY_ALBUM });
        curator
            .servarr
            .done(Method::Put, ARTIST_EDITOR, Some(edited.to_string()))
            .await?;
        let held: Vec<Held> = curator
            .servarr
            .read(&format!("{ALBUMS}?artistId={id}"), THE_ALBUMS)
            .await?;
        let ids: Vec<i64> = held.into_iter().map(|one| one.id).collect();
        if !ids.is_empty() {
            monitored(curator, &ids).await?;
        }
        if ask.search {
            commanded(curator, json!({ "name": "ArtistSearch", "artistId": id })).await?;
        }
        return standing(curator, ask).await;
    }
    let mut body = found;
    filed(&mut body, ask, EVERY_ALBUM);
    body.insert(
        "addOptions".to_owned(),
        json!({ "monitor": EVERY_ALBUM, "searchForMissingAlbums": ask.search }),
    );
    let added: Artist = added(curator, ARTISTS, body, THE_ARTISTS).await?;
    Ok(super::artist(added))
}

/// The resource the lookup at `path` answers for the reference `ask` names, as Lidarr
/// wrote it, matched on its `id_field`.
async fn resource(
    curator: &Curator,
    path: &str,
    id_field: &str,
    ask: &MusicAsk,
    what: &str,
) -> Result<Map<String, Value>, Failure> {
    let found: Vec<Value> = curator
        .looked_up(path, &format!("{BY_ID}{}", ask.reference), what)
        .await?;
    found
        .into_iter()
        .filter_map(|one| match one {
            Value::Object(one) => Some(one),
            _ => None,
        })
        .find(|one| one.get(id_field).and_then(Value::as_str) == Some(ask.reference.as_str()))
        .ok_or_else(|| curator.refused(NOT_IN_THE_CATALOGUE))
}

/// `resource` read as `T`; `what` names it where it cannot be.
fn decoded<T: serde::de::DeserializeOwned>(
    curator: &Curator,
    resource: Map<String, Value>,
    what: &str,
) -> Result<T, Failure> {
    serde_json::from_value(Value::Object(resource))
        .map_err(|_| curator.refused(&format!("{what} could not be read")))
}

/// `artist` filed under the root folder and profiles `ask` names, wanted, taking
/// `albums` of those that come later.
fn filed(artist: &mut Map<String, Value>, ask: &MusicAsk, albums: &str) {
    artist.insert("rootFolderPath".to_owned(), json!(ask.root_folder));
    artist.insert("qualityProfileId".to_owned(), json!(ask.quality_profile));
    artist.insert("metadataProfileId".to_owned(), json!(ask.metadata_profile));
    artist.insert("monitored".to_owned(), Value::Bool(true));
    artist.insert("monitorNewItems".to_owned(), json!(albums));
}

/// The albums `ids` names wanted.
async fn monitored(curator: &Curator, ids: &[i64]) -> Result<(), Failure> {
    let body = json!({ "albumIds": ids, "monitored": true });
    curator
        .servarr
        .done(Method::Put, ALBUM_MONITOR, Some(body.to_string()))
        .await
}

/// Lidarr told to run `command`.
async fn commanded(curator: &Curator, command: Value) -> Result<(), Failure> {
    curator
        .servarr
        .done(Method::Post, COMMAND, Some(command.to_string()))
        .await
}

/// What adding `body` at `path` came to, read as `T`.
async fn added<T: serde::de::DeserializeOwned>(
    curator: &Curator,
    path: &str,
    body: Map<String, Value>,
    what: &str,
) -> Result<T, Failure> {
    let body = Value::Object(body).to_string();
    let response = curator.servarr.ask(Method::Post, path, Some(body)).await?;
    let endpoint = curator.servarr.endpoint();
    endpoint.expect_success(&response)?;
    endpoint.decode(&response, &format!("{what} could not be read"))
}

/// The candidate `ask` names as it now stands.
async fn standing(curator: &Curator, ask: &MusicAsk) -> Result<Candidate, Failure> {
    curator
        .candidate(ask.unit, &ask.reference)
        .await?
        .ok_or_else(|| curator.refused(NOT_IN_THE_CATALOGUE))
}
