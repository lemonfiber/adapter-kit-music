//! Lidarr's v1 resources as its lookups answer them: an album, which is a release group,
//! and an artist, each with the parts a member is shown of it and how much of it the
//! house holds; and the root folders and profiles a request is made with.

use serde::Deserialize;

/// What Lidarr calls the picture a member is shown of an album or an artist.
const COVER: &str = "cover";

/// What Lidarr calls an artist's picture where it has no cover.
const POSTER: &str = "poster";

/// What an address published beyond the stack begins with.
const PUBLISHED: [&str; 2] = ["https://", "http://"];

/// How many digits a year is written in at the front of a date.
const YEAR_DIGITS: usize = 4;

/// One picture of an album or an artist.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    /// What the picture is of: a cover, a poster, a fan art.
    #[serde(default)]
    pub cover_type: String,
    /// Where the picture is published, beyond the stack.
    #[serde(default)]
    pub remote_url: Option<String>,
}

/// The artist an album is by, as an album names them.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Credited {
    /// What the artist is called.
    pub artist_name: String,
}

/// An album as Lidarr's lookup answers it.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    /// Its MusicBrainz release group id.
    pub foreign_album_id: String,
    /// What it is called.
    pub title: String,
    /// When it first came out, as an ISO 8601 date and time.
    #[serde(default)]
    pub release_date: Option<String>,
    /// Who it is by.
    #[serde(default)]
    pub artist: Option<Credited>,
    /// Lidarr's own id for it where it holds it.
    #[serde(default)]
    pub id: Option<i64>,
    /// Lidarr's own id for its artist where it holds the album.
    #[serde(default)]
    pub artist_id: Option<i64>,
    /// Whether Lidarr wants it.
    #[serde(default)]
    pub monitored: bool,
    /// Its pictures.
    #[serde(default)]
    pub images: Vec<Image>,
    /// How much of it the house holds, where Lidarr holds it.
    #[serde(default)]
    pub statistics: Option<Statistics>,
}

/// An artist as Lidarr's lookup answers it.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    /// Its MusicBrainz artist id.
    pub foreign_artist_id: String,
    /// What the artist is called.
    pub artist_name: String,
    /// Lidarr's own id for it where it holds it.
    #[serde(default)]
    pub id: Option<i64>,
    /// Whether Lidarr wants it.
    #[serde(default)]
    pub monitored: bool,
    /// Its pictures.
    #[serde(default)]
    pub images: Vec<Image>,
    /// How much of it the house holds, where Lidarr holds it.
    #[serde(default)]
    pub statistics: Option<Statistics>,
}

/// How much of an album or an artist the house holds.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Statistics {
    /// The tracks of it filed.
    #[serde(default)]
    pub track_file_count: u32,
    /// Every track it has, wanted or not.
    #[serde(default)]
    pub total_track_count: u32,
}

/// An album or an artist Lidarr holds, by its id, with how much of it the house holds.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Counted {
    /// Lidarr's own id for it.
    pub id: i64,
    /// How much of it the house holds.
    #[serde(default)]
    pub statistics: Option<Statistics>,
}

/// A quality or metadata profile.
#[derive(Debug, Clone, Deserialize)]
pub struct Named {
    /// Lidarr's id for it.
    pub id: i64,
    /// What it is called.
    #[serde(default)]
    pub name: String,
}

/// A root folder music is filed under.
#[derive(Debug, Clone, Deserialize)]
pub struct Folder {
    /// Where it is.
    pub path: String,
}

impl Album {
    /// The year it first came out, where its release date names one.
    #[must_use]
    pub fn year(&self) -> Option<u16> {
        self.release_date.as_deref().and_then(year_of)
    }

    /// Where its cover is published, where it has one.
    #[must_use]
    pub fn cover(&self) -> Option<&str> {
        pictured(&self.images, &[COVER])
    }

    /// Whether the house holds every track of it.
    #[must_use]
    pub fn whole(&self) -> bool {
        self.statistics.is_some_and(Statistics::whole)
    }
}

impl Statistics {
    /// Whether the house holds every track of it.
    #[must_use]
    pub const fn whole(self) -> bool {
        self.total_track_count > 0 && self.track_file_count >= self.total_track_count
    }

    /// Whether the house holds any track of it.
    #[must_use]
    pub const fn some(self) -> bool {
        self.track_file_count > 0
    }
}

impl Artist {
    /// Where its picture is published: its cover, or its poster where it has none.
    #[must_use]
    pub fn cover(&self) -> Option<&str> {
        pictured(&self.images, &[COVER, POSTER])
    }
}

/// The year at the front of `date`, where it is written there.
fn year_of(date: &str) -> Option<u16> {
    date.get(..YEAR_DIGITS)
        .filter(|year| year.bytes().all(|digit| digit.is_ascii_digit()))
        .and_then(|year| year.parse().ok())
}

/// The first of `images` of the first of `kinds` that has one, where it is published
/// beyond the stack.
fn pictured<'a>(images: &'a [Image], kinds: &[&str]) -> Option<&'a str> {
    kinds.iter().find_map(|kind| {
        images
            .iter()
            .filter(|image| image.cover_type.eq_ignore_ascii_case(kind))
            .filter_map(|image| image.remote_url.as_deref())
            .find(|url| PUBLISHED.iter().any(|scheme| url.starts_with(scheme)))
    })
}

#[cfg(test)]
mod tests;
