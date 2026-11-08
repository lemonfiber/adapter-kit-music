//! Lidarr's v1 resources as its lookups answer them: an album, which is a release group,
//! and an artist, each with the parts a member is shown of it.

use serde::Deserialize;

/// What Lidarr calls the picture a member is shown of an album or an artist.
const COVER: &str = "cover";

/// What Lidarr calls an artist's picture where it has no cover.
const POSTER: &str = "poster";

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
    /// Whether Lidarr wants it.
    #[serde(default)]
    pub monitored: bool,
    /// Its pictures.
    #[serde(default)]
    pub images: Vec<Image>,
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

/// The first of `images` of the first of `kinds` that has one, where it is published.
fn pictured<'a>(images: &'a [Image], kinds: &[&str]) -> Option<&'a str> {
    kinds.iter().find_map(|kind| {
        images
            .iter()
            .filter(|image| image.cover_type.eq_ignore_ascii_case(kind))
            .find_map(|image| image.remote_url.as_deref())
    })
}

#[cfg(test)]
mod tests;
