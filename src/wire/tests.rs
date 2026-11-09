use super::{Album, Artist};

fn album(json: &str) -> Album {
    let read = serde_json::from_str::<Album>(json);
    assert!(read.is_ok(), "{read:?}");
    read.unwrap_or_else(|_| unreachable!("asserted"))
}

fn artist(json: &str) -> Artist {
    let read = serde_json::from_str::<Artist>(json);
    assert!(read.is_ok(), "{read:?}");
    read.unwrap_or_else(|_| unreachable!("asserted"))
}

#[test]
fn an_album_names_its_year_and_its_cover() {
    let read = album(
        r#"{"foreignAlbumId":"9162580e","title":"Abbey Road","releaseDate":"1969-09-26T00:00:00Z",
            "artist":{"artistName":"The Beatles"},"monitored":false,
            "images":[{"coverType":"disc","remoteUrl":"https://example.invalid/disc.jpg"},
                      {"coverType":"Cover","remoteUrl":"https://example.invalid/cover.jpg"}]}"#,
    );

    assert_eq!(read.year(), Some(1969));
    assert_eq!(read.cover(), Some("https://example.invalid/cover.jpg"));
    assert_eq!(
        read.artist.map(|by| by.artist_name).as_deref(),
        Some("The Beatles")
    );
    assert_eq!(read.id, None);
}

#[test]
fn an_album_with_no_date_or_cover_names_neither() {
    for date in ["", r#","releaseDate":"unknown""#, r#","releaseDate":"19""#] {
        let read = album(&format!(
            r#"{{"foreignAlbumId":"a","title":"b","images":[{{"coverType":"cover"}}]{date}}}"#
        ));

        assert_eq!(read.year(), None, "{date}");
        assert_eq!(read.cover(), None, "{date}");
    }
}

#[test]
fn an_artist_is_pictured_by_its_cover_or_else_its_poster() {
    let postered = artist(
        r#"{"foreignArtistId":"b10bbbfc","artistName":"The Beatles","id":4,"monitored":true,
            "images":[{"coverType":"poster","remoteUrl":"https://example.invalid/poster.jpg"}]}"#,
    );
    let covered = artist(
        r#"{"foreignArtistId":"c","artistName":"d",
            "images":[{"coverType":"poster","remoteUrl":"https://example.invalid/poster.jpg"},
                      {"coverType":"cover","remoteUrl":"https://example.invalid/cover.jpg"}]}"#,
    );

    assert_eq!(postered.cover(), Some("https://example.invalid/poster.jpg"));
    assert!(postered.monitored && postered.id == Some(4));
    assert_eq!(covered.cover(), Some("https://example.invalid/cover.jpg"));
}

#[test]
fn a_picture_kept_only_inside_lidarr_is_not_published() {
    let held = artist(
        r#"{"foreignArtistId":"b10bbbfc","artistName":"The Beatles","id":4,
            "images":[{"coverType":"poster","remoteUrl":"/config/MediaCover/4/poster.jpg"}]}"#,
    );
    let plain = artist(
        r#"{"foreignArtistId":"c","artistName":"d",
            "images":[{"coverType":"poster","remoteUrl":"http://example.invalid/poster.jpg"}]}"#,
    );

    assert_eq!(held.cover(), None);
    assert_eq!(plain.cover(), Some("http://example.invalid/poster.jpg"));
}
