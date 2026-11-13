//! `music.curate` as a music curator speaking Lidarr's v1 API answers it: albums and
//! artists found by words a page at a time, one by its MusicBrainz id, asked for at a
//! root folder and profiles it holds, those root folders and profiles, and one it holds
//! removed with its files.

use adapter_kit_servarr::client::Servarr;
use async_trait::async_trait;
use lemonfiber_adapter_kit::endpoint::form_encoded;
use lemonfiber_ports::service::{
    AskedWith, Candidate, CandidateState, Candidates, Failure, MusicAsk, MusicCurating, MusicUnit,
    Profile,
};
use serde::de::DeserializeOwned;

use crate::wire::{Album, Artist, Counted, Folder, Named, Statistics};

mod removing;
mod requesting;

/// How many candidates one page of a lookup holds.
const PAGE: usize = 20;

/// What Lidarr reads a MusicBrainz id after, in place of words, in a lookup.
const BY_ID: &str = "lidarr:";

/// Where Lidarr lists the albums it holds.
const ALBUMS: &str = "/album";

/// Where Lidarr lists the artists it holds.
const ARTISTS: &str = "/artist";

/// What Lidarr reads each id of the albums it lists after.
const ALBUM_IDS: &str = "albumIds";

/// Where Lidarr looks albums up.
const ALBUM_LOOKUP: &str = "/album/lookup";

/// Where Lidarr looks artists up.
const ARTIST_LOOKUP: &str = "/artist/lookup";

/// Where Lidarr lists its root folders.
const ROOT_FOLDERS: &str = "/rootfolder";

/// Where Lidarr lists its quality profiles.
const QUALITY_PROFILES: &str = "/qualityprofile";

/// Where Lidarr lists its metadata profiles.
const METADATA_PROFILES: &str = "/metadataprofile";

/// What a lookup of albums is called where it cannot be read.
const THE_ALBUMS: &str = "the albums";

/// What a lookup of artists is called where it cannot be read.
const THE_ARTISTS: &str = "the artists";

/// Said of a page that begins nowhere a lookup gives.
const NO_SUCH_PAGE: &str = "that page begins nowhere a lookup gives";

/// `music.curate`, answered by a music curator through `servarr`.
pub struct Curator {
    servarr: Servarr,
}

impl Curator {
    /// The music curator `servarr` speaks to.
    #[must_use]
    pub const fn new(servarr: Servarr) -> Self {
        Self { servarr }
    }

    /// The albums the lookup of `term` finds, as the curator orders them.
    async fn albums(&self, term: &str) -> Result<Vec<Album>, Failure> {
        self.looked_up(ALBUM_LOOKUP, term, THE_ALBUMS).await
    }

    /// The artists the lookup of `term` finds, as the curator orders them.
    async fn artists(&self, term: &str) -> Result<Vec<Artist>, Failure> {
        self.looked_up(ARTIST_LOOKUP, term, THE_ARTISTS).await
    }

    /// `found` as candidates, each it holds with how much of it the house holds, which
    /// its lookup does not say.
    async fn albums_standing(&self, mut found: Vec<Album>) -> Result<Vec<Candidate>, Failure> {
        let held: Vec<String> = found
            .iter()
            .filter_map(|one| one.id)
            .map(|id| format!("{ALBUM_IDS}={id}"))
            .collect();
        if !held.is_empty() {
            let counted: Vec<Counted> = self
                .servarr
                .read(&format!("{ALBUMS}?{}", held.join("&")), THE_ALBUMS)
                .await?;
            for one in &mut found {
                one.statistics = statistics_of(&counted, one.id);
            }
        }
        Ok(found.into_iter().map(album).collect())
    }

    /// `found` as candidates, each it holds with how much of it the house holds, which
    /// its lookup does not say.
    async fn artists_standing(&self, mut found: Vec<Artist>) -> Result<Vec<Candidate>, Failure> {
        for one in &mut found {
            if let Some(id) = one.id {
                let counted: Counted = self
                    .servarr
                    .read(&format!("{ARTISTS}/{id}"), THE_ARTISTS)
                    .await?;
                one.statistics = counted.statistics;
            }
        }
        Ok(found.into_iter().map(artist).collect())
    }

    /// What the lookup at `path` answers for `term`, read as `T`.
    async fn looked_up<T: DeserializeOwned>(
        &self,
        path: &str,
        term: &str,
        what: &str,
    ) -> Result<Vec<T>, Failure> {
        let query = form_encoded(&[("term", term)]);
        self.servarr.read(&format!("{path}?{query}"), what).await
    }

    /// A refusal saying `detail`.
    fn refused(&self, detail: &str) -> Failure {
        self.servarr.endpoint().refused(detail)
    }
}

#[async_trait]
impl MusicCurating for Curator {
    async fn lookup(
        &self,
        unit: MusicUnit,
        term: &str,
        after: Option<&str>,
    ) -> Result<Candidates, Failure> {
        let from = after.map_or(Ok(0), |after| {
            after
                .parse::<usize>()
                .map_err(|_| self.refused(NO_SUCH_PAGE))
        })?;
        Ok(match unit {
            MusicUnit::Album => {
                let (page, next) = paged(self.albums(term).await?, from);
                Candidates {
                    found: self.albums_standing(page).await?,
                    next,
                }
            }
            MusicUnit::Artist => {
                let (page, next) = paged(self.artists(term).await?, from);
                Candidates {
                    found: self.artists_standing(page).await?,
                    next,
                }
            }
        })
    }

    async fn candidate(
        &self,
        unit: MusicUnit,
        reference: &str,
    ) -> Result<Option<Candidate>, Failure> {
        let term = format!("{BY_ID}{reference}");
        Ok(match unit {
            MusicUnit::Album => {
                let one = self
                    .albums(&term)
                    .await?
                    .into_iter()
                    .filter(|one| one.foreign_album_id == reference)
                    .take(1)
                    .collect();
                self.albums_standing(one).await?.pop()
            }
            MusicUnit::Artist => {
                let one = self
                    .artists(&term)
                    .await?
                    .into_iter()
                    .filter(|one| one.foreign_artist_id == reference)
                    .take(1)
                    .collect();
                self.artists_standing(one).await?.pop()
            }
        })
    }

    async fn request(&self, ask: &MusicAsk) -> Result<Candidate, Failure> {
        let with = self.profiles().await?;
        if !with.root_folders.contains(&ask.root_folder) {
            return Err(self.refused("it holds no such root folder"));
        }
        if !with.quality.iter().any(|one| one.id == ask.quality_profile) {
            return Err(self.refused("it holds no such quality profile"));
        }
        if !with
            .metadata
            .iter()
            .any(|one| one.id == ask.metadata_profile)
        {
            return Err(self.refused("it holds no such metadata profile"));
        }
        match ask.unit {
            MusicUnit::Album => requesting::album(self, ask).await,
            MusicUnit::Artist => requesting::artist(self, ask).await,
        }
    }

    async fn profiles(&self) -> Result<AskedWith, Failure> {
        let folders: Vec<Folder> = self.servarr.read(ROOT_FOLDERS, "the root folders").await?;
        let quality: Vec<Named> = self
            .servarr
            .read(QUALITY_PROFILES, "the quality profiles")
            .await?;
        let metadata: Vec<Named> = self
            .servarr
            .read(METADATA_PROFILES, "the metadata profiles")
            .await?;
        Ok(AskedWith {
            root_folders: folders.into_iter().map(|one| one.path).collect(),
            quality: quality.into_iter().map(profile).collect(),
            metadata: metadata.into_iter().map(profile).collect(),
            units: vec![MusicUnit::Album.into(), MusicUnit::Artist.into()],
        })
    }

    async fn remove(&self, unit: MusicUnit, reference: &str) -> Result<(), Failure> {
        match unit {
            MusicUnit::Album => removing::album(self, reference).await,
            MusicUnit::Artist => removing::artist(self, reference).await,
        }
    }
}

/// An album as a candidate: here where the house holds every track, asked where Lidarr
/// wants it.
fn album(found: Album) -> Candidate {
    let state = if found.whole() {
        CandidateState::Here
    } else {
        wanted(found.id, found.monitored)
    };
    Candidate {
        name: found.title.clone(),
        by: found.artist.as_ref().map(|by| by.artist_name.clone()),
        year: found.year(),
        cover: found.cover().map(str::to_owned),
        format: None,
        state,
        reference: found.foreign_album_id,
    }
}

/// An artist as a candidate: here where the house holds every track of it, partly here
/// where it holds some, asked where Lidarr wants it.
fn artist(found: Artist) -> Candidate {
    let state = match found.statistics {
        Some(held) if held.whole() => CandidateState::Here,
        Some(held) if held.some() => CandidateState::PartlyHere,
        _ => wanted(found.id, found.monitored),
    };
    Candidate {
        name: found.artist_name.clone(),
        by: None,
        year: None,
        cover: found.cover().map(str::to_owned),
        format: None,
        state,
        reference: found.foreign_artist_id,
    }
}

/// Asked where Lidarr holds it and wants it, and nothing has asked for it otherwise.
const fn wanted(id: Option<i64>, monitored: bool) -> CandidateState {
    if id.is_some() && monitored {
        CandidateState::Asked
    } else {
        CandidateState::MayAsk
    }
}

/// The page of `found` beginning at `from`, and where the next begins where one does.
fn paged<T>(found: Vec<T>, from: usize) -> (Vec<T>, Option<String>) {
    let to = from.saturating_add(PAGE);
    let next = (found.len() > to).then(|| to.to_string());
    (found.into_iter().skip(from).take(PAGE).collect(), next)
}

/// How much of the one of `counted` with `id` the house holds.
fn statistics_of(counted: &[Counted], id: Option<i64>) -> Option<Statistics> {
    counted
        .iter()
        .find(|one| Some(one.id) == id)
        .and_then(|one| one.statistics)
}

/// A quality or metadata profile as a request names it.
fn profile(named: Named) -> Profile {
    Profile {
        id: named.id,
        name: named.name,
    }
}

#[cfg(test)]
mod tests;
