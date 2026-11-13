use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{MusicCurating, MusicUnit};

use crate::curator::tests::{curator, ok, refused};

const ABBEY_ROAD: &str = "9162580e-5df4-32de-80cc-f45a8d8a9b1d";

const BEATLES: &str = "b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d";

const THE_ALBUM: &str = "/album?foreignAlbumId=9162580e-5df4-32de-80cc-f45a8d8a9b1d";

const THEIR_ALBUMS: &str = "/album?artistId=4";

fn removals(fake: &Fake) -> Vec<String> {
    fake.requests()
        .into_iter()
        .filter(|one| one.method == Method::Delete)
        .map(|one| one.url)
        .collect()
}

fn album_with(theirs: &'static str) -> Arc<Fake> {
    Fake::by_rules(vec![
        (
            Some(Method::Get),
            THE_ALBUM,
            ok(
                r#"[{"id":7,"foreignAlbumId":"9162580e-5df4-32de-80cc-f45a8d8a9b1d","artistId":4,"monitored":true}]"#,
            ),
        ),
        (Some(Method::Get), THEIR_ALBUMS, ok(theirs)),
        (Some(Method::Delete), "/album/7?", ok("")),
        (Some(Method::Delete), "/artist/4?", ok("")),
    ])
}

#[tokio::test]
async fn an_album_is_removed_alone_with_its_files_where_its_artist_holds_more() {
    for theirs in [
        r#"[{"id":7,"monitored":true},{"id":8,"monitored":true}]"#,
        r#"[{"id":7,"monitored":true},{"id":8,"statistics":{"trackFileCount":3,"totalTrackCount":12}}]"#,
    ] {
        let fake = album_with(theirs);

        let removed = curator(&fake).remove(MusicUnit::Album, ABBEY_ROAD).await;

        assert!(removed.is_ok(), "{removed:?}");
        assert_eq!(
            removals(&fake),
            ["http://lidarr:8686/api/v1/album/7?deleteFiles=true&addImportListExclusion=false"],
            "{theirs}"
        );
    }
}

#[tokio::test]
async fn an_album_takes_its_artist_with_it_where_nothing_else_of_theirs_is_held() {
    let fake = album_with(
        r#"[{"id":7,"monitored":true},{"id":8,"monitored":false},
            {"id":9,"statistics":{"trackFileCount":0,"totalTrackCount":12}}]"#,
    );

    let removed = curator(&fake).remove(MusicUnit::Album, ABBEY_ROAD).await;

    assert!(removed.is_ok(), "{removed:?}");
    assert_eq!(
        removals(&fake),
        [
            "http://lidarr:8686/api/v1/album/7?deleteFiles=true&addImportListExclusion=false",
            "http://lidarr:8686/api/v1/artist/4?deleteFiles=true&addImportListExclusion=false"
        ]
    );
}

#[tokio::test]
async fn an_album_naming_no_artist_is_removed_alone() {
    let fake = Fake::by_rules(vec![
        (
            Some(Method::Get),
            THE_ALBUM,
            ok(r#"[{"id":7,"foreignAlbumId":"9162580e-5df4-32de-80cc-f45a8d8a9b1d"}]"#),
        ),
        (Some(Method::Delete), "/album/7?", ok("")),
    ]);

    let removed = curator(&fake).remove(MusicUnit::Album, ABBEY_ROAD).await;

    assert!(removed.is_ok(), "{removed:?}");
    assert_eq!(removals(&fake).len(), 1);
    assert!(!fake.asked_for("artistId"));
}

#[tokio::test]
async fn an_album_lidarr_does_not_hold_exactly_is_refused_and_nothing_is_removed() {
    for found in [
        "[]",
        r#"[{"id":7,"foreignAlbumId":"9162580e-5df4-32de-80cc-f45a8d8a9b1d"},
            {"id":8,"foreignAlbumId":"9162580e-5df4-32de-80cc-f45a8d8a9b1d"}]"#,
        r#"[{"id":7,"foreignAlbumId":"1b022e01-4da6-387b-8658-8678046e4cef"}]"#,
        r#"[{"id":7}]"#,
    ] {
        let fake = Fake::by_rules(vec![
            (Some(Method::Get), THE_ALBUM, ok(found)),
            (None, "/", ok("")),
        ]);

        let removed = curator(&fake).remove(MusicUnit::Album, ABBEY_ROAD).await;

        assert_eq!(refused(removed.err()), "it holds no such album", "{found}");
        assert!(removals(&fake).is_empty(), "{found}");
        assert!(!fake.asked_for("artistId"), "{found}");
    }
}

#[tokio::test]
async fn an_album_reference_that_is_no_musicbrainz_id_is_refused_unasked() {
    let fake = Fake::always(ok("[]"));

    let removed = curator(&fake).remove(MusicUnit::Album, "conformance").await;

    assert_eq!(refused(removed.err()), "it holds no such album");
    assert!(fake.requests().is_empty());
}

#[tokio::test]
async fn an_album_lidarr_will_not_remove_keeps_its_artist() {
    let fake = Fake::by_rules(vec![
        (
            Some(Method::Get),
            THE_ALBUM,
            ok(
                r#"[{"id":7,"foreignAlbumId":"9162580e-5df4-32de-80cc-f45a8d8a9b1d","artistId":4}]"#,
            ),
        ),
        (Some(Method::Get), THEIR_ALBUMS, ok(r#"[{"id":7}]"#)),
        (Some(Method::Delete), "/album/7?", Answer::reply(500, "")),
    ]);

    let removed = curator(&fake).remove(MusicUnit::Album, ABBEY_ROAD).await;

    assert!(removed.is_err());
    assert_eq!(removals(&fake).len(), 1);
}

#[tokio::test]
async fn an_artist_is_removed_with_every_album_and_its_files() {
    let fake = Fake::by_rules(vec![
        (
            Some(Method::Get),
            "/artist?mbId=b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d",
            ok(
                r#"[{"id":4,"foreignArtistId":"b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d","artistName":"The Beatles"}]"#,
            ),
        ),
        (Some(Method::Delete), "/artist/4?", ok("")),
    ]);

    let removed = curator(&fake).remove(MusicUnit::Artist, BEATLES).await;

    assert!(removed.is_ok(), "{removed:?}");
    assert_eq!(
        removals(&fake),
        ["http://lidarr:8686/api/v1/artist/4?deleteFiles=true&addImportListExclusion=false"]
    );
}

#[tokio::test]
async fn an_artist_lidarr_does_not_hold_exactly_is_refused_and_nothing_is_removed() {
    for found in [
        "[]",
        r#"[{"id":4,"foreignArtistId":"b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d","artistName":"a"},
            {"id":5,"foreignArtistId":"b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d","artistName":"b"}]"#,
        r#"[{"id":4,"foreignArtistId":"83d91898-7763-47d7-b03b-b92132375c47","artistName":"a"}]"#,
        r#"[{"foreignArtistId":"b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d","artistName":"a"}]"#,
    ] {
        let fake = Fake::by_rules(vec![
            (Some(Method::Get), "/artist?mbId=", ok(found)),
            (None, "/", ok("")),
        ]);

        let removed = curator(&fake).remove(MusicUnit::Artist, BEATLES).await;

        assert_eq!(refused(removed.err()), "it holds no such artist", "{found}");
        assert!(removals(&fake).is_empty(), "{found}");
    }
}

#[tokio::test]
async fn an_artist_reference_that_is_no_musicbrainz_id_is_refused_unasked() {
    let fake = Fake::always(ok("[]"));
    let curator = curator(&fake);

    for reference in [
        "conformance",
        "b10bbbfc-cf9e-42e0-be17",
        "b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d-0",
        "b10bbbf-ccf9e-42e0-be17-e2c3e1d2600d",
        "b10bbbfg-cf9e-42e0-be17-e2c3e1d2600d",
    ] {
        let removed = curator.remove(MusicUnit::Artist, reference).await;

        assert_eq!(
            refused(removed.err()),
            "it holds no such artist",
            "{reference}"
        );
    }
    assert!(fake.requests().is_empty());
}
