use std::sync::Arc;

use adapter_kit_servarr::client::Servarr;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{
    CandidateState, CuratedUnit, Failure, MusicAsk, MusicCurating, MusicUnit, Profile,
};
use serde_json::Value;

use super::Curator;

const BASE: &str = "http://lidarr:8686";

const ABBEY_ROAD: &str = "9162580e";

const BEATLES: &str = "b10bbbfc";

fn curator(fake: &Arc<Fake>) -> Curator {
    Curator::new(Servarr::new(
        fake.clone(),
        BASE,
        Arc::new("the-key".to_owned()),
        "lidarr",
        1,
    ))
}

fn ok(body: &str) -> Answer {
    Answer::reply(200, body.to_owned())
}

fn album(reference: &str, held: &str) -> String {
    format!(
        r#"{{"foreignAlbumId":"{reference}","title":"Abbey Road","releaseDate":"1969-09-26T00:00:00Z",
            "artist":{{"artistName":"The Beatles","foreignArtistId":"{BEATLES}"}},
            "images":[{{"coverType":"cover","remoteUrl":"https://example.invalid/cover.jpg"}}]{held}}}"#
    )
}

fn artist(held: &str) -> String {
    format!(
        r#"{{"foreignArtistId":"{BEATLES}","artistName":"The Beatles",
            "images":[{{"coverType":"poster","remoteUrl":"https://example.invalid/poster.jpg"}}]{held}}}"#
    )
}

fn listed(one: &str) -> String {
    format!("[{one}]")
}

const PROFILES: [(&str, &str); 3] = [
    ("/rootfolder", r#"[{"path":"/music"}]"#),
    ("/qualityprofile", r#"[{"id":1,"name":"Lossless"}]"#),
    ("/metadataprofile", r#"[{"id":2,"name":"Standard"}]"#),
];

fn routed(routes: Vec<(Option<Method>, &'static str, Answer)>) -> Arc<Fake> {
    let mut every: Vec<(Option<Method>, &'static str, Answer)> = PROFILES
        .iter()
        .map(|(path, body)| (Some(Method::Get), *path, ok(body)))
        .collect();
    every.extend(routes);
    Fake::by_rules(every)
}

fn asking(unit: MusicUnit, reference: &str, search: bool) -> MusicAsk {
    MusicAsk {
        unit,
        reference: reference.to_owned(),
        root_folder: "/music".to_owned(),
        quality_profile: 1,
        metadata_profile: 2,
        search,
    }
}

fn sent(fake: &Fake, method: Method, fragment: &str) -> Vec<Value> {
    fake.requests()
        .into_iter()
        .filter(|one| one.method == method && one.url.contains(fragment))
        .filter_map(|one| one.body.and_then(|body| serde_json::from_str(&body).ok()))
        .collect()
}

fn at(value: Option<&Value>, pointer: &str) -> Value {
    value
        .and_then(|value| value.pointer(pointer))
        .cloned()
        .unwrap_or_default()
}

fn refused(failure: Option<Failure>) -> String {
    match failure {
        Some(Failure::Refused { detail, .. }) => detail,
        other => format!("not a refusal: {other:?}"),
    }
}

#[tokio::test]
async fn a_lookup_answers_a_page_at_a_time_and_says_where_the_next_begins() {
    let many = (0..25)
        .map(|n| album(&format!("album-{n}"), ""))
        .collect::<Vec<_>>()
        .join(",");
    let fake = Fake::always(ok(&format!("[{many}]")));
    let curator = curator(&fake);

    let first = curator
        .lookup(MusicUnit::Album, "abbey road", None)
        .await
        .ok();
    let second = curator
        .lookup(MusicUnit::Album, "abbey road", Some("20"))
        .await
        .ok();

    assert_eq!(first.as_ref().map(|page| page.found.len()), Some(20));
    assert_eq!(first.and_then(|page| page.next).as_deref(), Some("20"));
    assert_eq!(second.as_ref().map(|page| page.found.len()), Some(5));
    assert_eq!(second.and_then(|page| page.next), None);
    assert!(fake.asked_for("/api/v1/album/lookup?term=abbey+road"));
}

#[tokio::test]
async fn a_page_beginning_nowhere_a_lookup_gives_is_refused() {
    let fake = Fake::always(ok("[]"));

    let page = curator(&fake)
        .lookup(MusicUnit::Album, "abbey road", Some("later"))
        .await;

    assert_eq!(
        refused(page.err()),
        "that page begins nowhere a lookup gives"
    );
    assert!(fake.requests().is_empty());
}

#[tokio::test]
async fn an_album_is_here_where_every_track_is_filed_asked_where_wanted_and_askable_otherwise() {
    let held = [
        album("whole", r#","id":1,"monitored":true"#),
        album("wanted", r#","id":2,"monitored":true"#),
        album("unwanted", r#","id":3,"monitored":false"#),
        album("new", ""),
    ]
    .join(",");
    let fake = Fake::by_rules(vec![
        (Some(Method::Get), "/album/lookup", ok(&format!("[{held}]"))),
        (
            Some(Method::Get),
            "/album?",
            ok(
                r#"[{"id":1,"statistics":{"trackFileCount":17,"totalTrackCount":17}},
                   {"id":2,"statistics":{"trackFileCount":3,"totalTrackCount":17}},
                   {"id":3}]"#,
            ),
        ),
    ]);

    let found = curator(&fake)
        .lookup(MusicUnit::Album, "abbey road", None)
        .await
        .map(|page| page.found)
        .unwrap_or_default();

    assert_eq!(
        found.iter().map(|one| one.state).collect::<Vec<_>>(),
        [
            CandidateState::Here,
            CandidateState::Asked,
            CandidateState::MayAsk,
            CandidateState::MayAsk
        ]
    );
    assert!(fake.asked_for("/api/v1/album?albumIds=1&albumIds=2&albumIds=3"));
    let first = found.first();
    assert_eq!(first.map(|one| one.name.as_str()), Some("Abbey Road"));
    assert_eq!(first.and_then(|one| one.by.as_deref()), Some("The Beatles"));
    assert_eq!(first.and_then(|one| one.year), Some(1969));
    assert_eq!(
        first.and_then(|one| one.cover.as_deref()),
        Some("https://example.invalid/cover.jpg")
    );
}

#[tokio::test]
async fn a_page_lidarr_holds_none_of_asks_for_nothing_more() {
    let fake = Fake::always(ok(&listed(&album("new", ""))));

    let found = curator(&fake)
        .lookup(MusicUnit::Album, "abbey road", None)
        .await
        .map(|page| page.found)
        .unwrap_or_default();

    assert_eq!(found.len(), 1);
    assert_eq!(fake.requests().len(), 1);
}

#[tokio::test]
async fn an_artist_is_here_partly_here_or_asked_by_what_the_house_holds_of_it() {
    let fake = Fake::by_rules(vec![
        (
            Some(Method::Get),
            "/artist/lookup",
            ok(&format!(
                "[{},{},{}]",
                artist(r#","id":4,"monitored":true"#),
                artist(r#","id":5,"monitored":true"#),
                artist(r#","id":6,"monitored":true"#)
            )),
        ),
        (
            Some(Method::Get),
            "/artist/4",
            ok(r#"{"id":4,"statistics":{"trackFileCount":46,"totalTrackCount":46}}"#),
        ),
        (
            Some(Method::Get),
            "/artist/5",
            ok(r#"{"id":5,"statistics":{"trackFileCount":12,"totalTrackCount":46}}"#),
        ),
        (
            Some(Method::Get),
            "/artist/6",
            ok(r#"{"id":6,"statistics":{"trackFileCount":0,"totalTrackCount":46}}"#),
        ),
    ]);

    let found = curator(&fake)
        .lookup(MusicUnit::Artist, "beatles", None)
        .await
        .map(|page| page.found)
        .unwrap_or_default();

    assert_eq!(
        found.iter().map(|one| one.state).collect::<Vec<_>>(),
        [
            CandidateState::Here,
            CandidateState::PartlyHere,
            CandidateState::Asked
        ]
    );
    let first = found.first();
    assert_eq!(first.map(|one| one.reference.as_str()), Some(BEATLES));
    assert_eq!(
        first.and_then(|one| one.cover.as_deref()),
        Some("https://example.invalid/poster.jpg")
    );
    assert!(fake.asked_for("/api/v1/artist/lookup?term=beatles"));
}

#[tokio::test]
async fn an_artist_page_answers_a_page_at_a_time() {
    let many = (0..21).map(|_| artist("")).collect::<Vec<_>>().join(",");
    let fake = Fake::always(ok(&format!("[{many}]")));

    let page = curator(&fake)
        .lookup(MusicUnit::Artist, "beatles", None)
        .await
        .ok();

    assert_eq!(page.as_ref().map(|page| page.found.len()), Some(20));
    assert_eq!(page.and_then(|page| page.next).as_deref(), Some("20"));
}

#[tokio::test]
async fn one_artist_is_read_by_its_reference_with_what_the_house_holds_of_it() {
    let fake = Fake::by_rules(vec![
        (
            Some(Method::Get),
            "/artist/lookup",
            ok(&listed(&artist(r#","id":4,"monitored":false"#))),
        ),
        (
            Some(Method::Get),
            "/artist/4",
            ok(r#"{"id":4,"statistics":{"trackFileCount":1,"totalTrackCount":46}}"#),
        ),
    ]);
    let curator = curator(&fake);

    let found = curator.candidate(MusicUnit::Artist, BEATLES).await.ok();
    let missing = curator.candidate(MusicUnit::Artist, "absent").await.ok();

    assert_eq!(
        found.flatten().map(|one| one.state),
        Some(CandidateState::PartlyHere)
    );
    assert_eq!(missing, Some(None));
    assert!(fake.asked_for("term=lidarr%3Ab10bbbfc"));
}

#[tokio::test]
async fn one_candidate_is_read_by_its_reference_and_nothing_where_the_catalogue_holds_none() {
    let fake = Fake::always(ok(&format!(
        "[{},{}]",
        album("another", ""),
        album(ABBEY_ROAD, "")
    )));
    let curator = curator(&fake);

    let found = curator.candidate(MusicUnit::Album, ABBEY_ROAD).await.ok();
    let missing = curator.candidate(MusicUnit::Album, "absent").await.ok();

    assert_eq!(
        found.flatten().map(|one| one.reference),
        Some(ABBEY_ROAD.to_owned())
    );
    assert_eq!(missing, Some(None));
    assert!(fake.asked_for("term=lidarr%3A9162580e"));
}

#[tokio::test]
async fn the_profiles_are_the_root_folders_quality_and_metadata_profiles_it_holds() {
    let fake = routed(Vec::new());

    let with = curator(&fake).profiles().await.ok();

    assert_eq!(
        with.as_ref().map(|with| with.root_folders.clone()),
        Some(vec!["/music".to_owned()])
    );
    assert_eq!(
        with.as_ref().map(|with| with.quality.clone()),
        Some(vec![Profile {
            id: 1,
            name: "Lossless".to_owned()
        }])
    );
    assert_eq!(
        with.as_ref().map(|with| with.metadata.clone()),
        Some(vec![Profile {
            id: 2,
            name: "Standard".to_owned()
        }])
    );
    assert_eq!(
        with.map(|with| with.units),
        Some(vec![CuratedUnit::Album, CuratedUnit::Artist])
    );
}

#[tokio::test]
async fn a_request_naming_what_it_does_not_hold_is_refused_and_sends_nothing() {
    let fake = routed(Vec::new());
    let curator = curator(&fake);
    let ask = asking(MusicUnit::Album, ABBEY_ROAD, false);

    let folder = curator
        .request(&MusicAsk {
            root_folder: "/elsewhere".to_owned(),
            ..ask.clone()
        })
        .await;
    let quality = curator
        .request(&MusicAsk {
            quality_profile: 9,
            ..ask.clone()
        })
        .await;
    let metadata = curator
        .request(&MusicAsk {
            metadata_profile: 9,
            ..ask
        })
        .await;

    assert_eq!(refused(folder.err()), "it holds no such root folder");
    assert_eq!(refused(quality.err()), "it holds no such quality profile");
    assert_eq!(refused(metadata.err()), "it holds no such metadata profile");
    assert!(fake.requests().iter().all(|one| one.method == Method::Get));
}

#[tokio::test]
async fn an_album_lidarr_holds_none_of_is_added_alone_under_its_artist_and_searched_for() {
    let fake = Fake::by_route_in_turn(
        PROFILES
            .iter()
            .map(|(path, body)| (Method::Get, *path, vec![ok(body)]))
            .chain([
                (
                    Method::Get,
                    "/album/lookup",
                    vec![
                        ok(&format!("[1,{}]", album(ABBEY_ROAD, ""))),
                        ok(&listed(&album(ABBEY_ROAD, r#","id":7,"monitored":true"#))),
                    ],
                ),
                (
                    Method::Get,
                    "/album?albumIds=7",
                    vec![ok(r#"[{"id":7,"statistics":{"totalTrackCount":17}}]"#)],
                ),
                (Method::Post, "/album", vec![ok(r#"{"id":7}"#)]),
                (Method::Post, "/command", vec![ok("{}")]),
            ])
            .collect(),
    );

    let asked = curator(&fake)
        .request(&asking(MusicUnit::Album, ABBEY_ROAD, true))
        .await;

    assert_eq!(asked.ok().map(|one| one.state), Some(CandidateState::Asked));
    let posted = sent(&fake, Method::Post, "/api/v1/album");
    let body = posted.first();
    assert_eq!(at(body, "/monitored"), Value::Bool(true));
    assert_eq!(
        at(body, "/addOptions/searchForNewAlbum"),
        Value::Bool(false)
    );
    assert_eq!(at(body, "/artist/artistName"), "The Beatles");
    assert_eq!(at(body, "/artist/rootFolderPath"), "/music");
    assert_eq!(at(body, "/artist/qualityProfileId"), 1);
    assert_eq!(at(body, "/artist/metadataProfileId"), 2);
    assert_eq!(at(body, "/artist/monitored"), Value::Bool(true));
    assert_eq!(at(body, "/artist/monitorNewItems"), "none");
    assert_eq!(at(body, "/artist/addOptions/monitor"), "unknown");
    assert_eq!(
        at(body, "/artist/addOptions/albumsToMonitor"),
        serde_json::json!([ABBEY_ROAD])
    );
    assert_eq!(
        at(body, "/artist/addOptions/searchForMissingAlbums"),
        Value::Bool(false)
    );
    let commands = sent(&fake, Method::Post, "/command");
    assert_eq!(at(commands.first(), "/name"), "AlbumSearch");
    assert_eq!(at(commands.first(), "/albumIds/0"), 7);
}

#[tokio::test]
async fn an_album_whose_artist_is_not_written_out_is_added_under_one_filed_as_asked() {
    let bare = r#"{"foreignAlbumId":"9162580e","title":"Abbey Road","artist":null}"#;
    let fake = routed(vec![
        (Some(Method::Get), "/album/lookup", ok(&listed(bare))),
        (Some(Method::Post), "/album", ok(r#"{"id":7}"#)),
    ]);

    let asked = curator(&fake)
        .request(&asking(MusicUnit::Album, ABBEY_ROAD, false))
        .await;

    assert_eq!(
        asked.ok().map(|one| one.state),
        Some(CandidateState::MayAsk)
    );
    let posted = sent(&fake, Method::Post, "/api/v1/album");
    assert_eq!(at(posted.first(), "/artist/rootFolderPath"), "/music");
    assert!(sent(&fake, Method::Post, "/command").is_empty());
}

#[tokio::test]
async fn an_album_lidarr_holds_is_wanted_searched_for_where_asked_and_answered_as_it_stands() {
    let fake = Fake::by_rules(
        PROFILES
            .iter()
            .map(|(path, body)| (Some(Method::Get), *path, ok(body)))
            .chain([
                (Some(Method::Put), "/album/monitor", ok("[]")),
                (Some(Method::Put), "/artist/editor", ok("[]")),
                (Some(Method::Post), "/command", ok("{}")),
                (
                    Some(Method::Get),
                    "/album?albumIds=7",
                    ok(r#"[{"id":7,"statistics":{"trackFileCount":0,"totalTrackCount":17}}]"#),
                ),
                (
                    Some(Method::Get),
                    "/album/lookup",
                    ok(&listed(&album(
                        ABBEY_ROAD,
                        r#","id":7,"artistId":4,"monitored":false"#,
                    ))),
                ),
            ])
            .collect(),
    );
    let curator = curator(&fake);

    let searched = curator
        .request(&asking(MusicUnit::Album, ABBEY_ROAD, true))
        .await;
    let quiet = curator
        .request(&asking(MusicUnit::Album, ABBEY_ROAD, false))
        .await;

    assert!(searched.is_ok() && quiet.is_ok());
    let monitored = sent(&fake, Method::Put, "/album/monitor");
    assert_eq!(monitored.len(), 2);
    assert_eq!(at(monitored.first(), "/albumIds/0"), 7);
    assert_eq!(at(monitored.first(), "/monitored"), Value::Bool(true));
    let commands = sent(&fake, Method::Post, "/command");
    assert_eq!(commands.len(), 1);
    assert_eq!(at(commands.first(), "/name"), "AlbumSearch");
    assert_eq!(at(commands.first(), "/albumIds/0"), 7);
    assert!(sent(&fake, Method::Post, "/api/v1/album").is_empty());
    let edited = sent(&fake, Method::Put, "/artist/editor");
    assert_eq!(at(edited.first(), "/artistIds/0"), 4);
    assert_eq!(at(edited.first(), "/monitored"), Value::Bool(true));
    assert_eq!(at(edited.first(), "/monitorNewItems"), Value::Null);
}

#[tokio::test]
async fn an_artist_lidarr_holds_none_of_is_added_with_every_album_and_searched_for() {
    let fake = routed(vec![
        (
            Some(Method::Get),
            "/artist/lookup",
            ok(&listed(&artist(""))),
        ),
        (
            Some(Method::Post),
            "/artist",
            ok(&artist(r#","id":4,"monitored":true"#)),
        ),
    ]);

    let asked = curator(&fake)
        .request(&asking(MusicUnit::Artist, BEATLES, true))
        .await;

    assert_eq!(asked.ok().map(|one| one.state), Some(CandidateState::Asked));
    let posted = sent(&fake, Method::Post, "/api/v1/artist");
    let body = posted.first();
    assert_eq!(at(body, "/artistName"), "The Beatles");
    assert_eq!(at(body, "/rootFolderPath"), "/music");
    assert_eq!(at(body, "/monitorNewItems"), "all");
    assert_eq!(at(body, "/addOptions/monitor"), "all");
    assert_eq!(
        at(body, "/addOptions/searchForMissingAlbums"),
        Value::Bool(true)
    );
}

#[tokio::test]
async fn an_artist_lidarr_holds_is_wanted_with_every_album_it_holds_and_searched_for() {
    let fake = routed(vec![
        (
            Some(Method::Get),
            "/artist/lookup",
            ok(&listed(&artist(r#","id":4,"monitored":false"#))),
        ),
        (Some(Method::Put), "/artist/editor", ok("[]")),
        (
            Some(Method::Get),
            "/album?artistId=4",
            ok(r#"[{"id":7},{"id":8}]"#),
        ),
        (Some(Method::Put), "/album/monitor", ok("[]")),
        (Some(Method::Post), "/command", ok("{}")),
        (Some(Method::Get), "/artist/4", ok(r#"{"id":4}"#)),
    ]);

    let asked = curator(&fake)
        .request(&asking(MusicUnit::Artist, BEATLES, true))
        .await;

    assert!(asked.is_ok(), "{asked:?}");
    let edited = sent(&fake, Method::Put, "/artist/editor");
    assert_eq!(at(edited.first(), "/artistIds/0"), 4);
    assert_eq!(at(edited.first(), "/monitored"), Value::Bool(true));
    assert_eq!(at(edited.first(), "/monitorNewItems"), "all");
    let monitored = sent(&fake, Method::Put, "/album/monitor");
    assert_eq!(
        at(monitored.first(), "/albumIds"),
        serde_json::json!([7, 8])
    );
    let commands = sent(&fake, Method::Post, "/command");
    assert_eq!(at(commands.first(), "/name"), "ArtistSearch");
    assert_eq!(at(commands.first(), "/artistId"), 4);
}

#[tokio::test]
async fn an_artist_lidarr_holds_is_not_searched_for_where_not_asked() {
    let fake = routed(vec![
        (
            Some(Method::Get),
            "/artist/lookup",
            ok(&listed(&artist(r#","id":4,"monitored":true"#))),
        ),
        (Some(Method::Put), "/artist/editor", ok("[]")),
        (Some(Method::Get), "/album?artistId=4", ok("[]")),
        (Some(Method::Put), "/album/monitor", ok("[]")),
        (Some(Method::Get), "/artist/4", ok(r#"{"id":4}"#)),
    ]);

    let asked = curator(&fake)
        .request(&asking(MusicUnit::Artist, BEATLES, false))
        .await;

    assert_eq!(asked.ok().map(|one| one.state), Some(CandidateState::Asked));
    assert!(sent(&fake, Method::Post, "/command").is_empty());
    assert!(sent(&fake, Method::Put, "/album/monitor").is_empty());
}

#[tokio::test]
async fn a_reference_the_catalogue_does_not_hold_is_refused() {
    let fake = routed(vec![
        (
            Some(Method::Get),
            "/album/lookup",
            ok(&listed(&album("another", ""))),
        ),
        (Some(Method::Get), "/artist/lookup", ok("[]")),
    ]);
    let curator = curator(&fake);

    let album = curator
        .request(&asking(MusicUnit::Album, ABBEY_ROAD, false))
        .await;
    let artist = curator
        .request(&asking(MusicUnit::Artist, BEATLES, false))
        .await;

    assert_eq!(
        refused(album.err()),
        "its catalogue holds no such reference"
    );
    assert_eq!(
        refused(artist.err()),
        "its catalogue holds no such reference"
    );
}

#[tokio::test]
async fn a_held_candidate_gone_from_the_catalogue_once_wanted_is_refused() {
    let fake = Fake::by_route_in_turn(
        PROFILES
            .iter()
            .map(|(path, body)| (Method::Get, *path, vec![ok(body)]))
            .chain([
                (Method::Put, "/album/monitor", vec![ok("[]")]),
                (
                    Method::Get,
                    "/album/lookup",
                    vec![ok(&listed(&album(ABBEY_ROAD, r#","id":7"#))), ok("[]")],
                ),
            ])
            .collect(),
    );

    let asked = curator(&fake)
        .request(&asking(MusicUnit::Album, ABBEY_ROAD, false))
        .await;

    assert_eq!(
        refused(asked.err()),
        "its catalogue holds no such reference"
    );
}

#[tokio::test]
async fn a_resource_that_does_not_read_or_an_addition_refused_is_said_so() {
    let unread = Fake::by_rules(
        PROFILES
            .iter()
            .map(|(path, body)| (Some(Method::Get), *path, ok(body)))
            .chain([(
                Some(Method::Get),
                "/album/lookup",
                ok(r#"[{"foreignAlbumId":"9162580e"}]"#),
            )])
            .collect(),
    );
    let turned_down = routed(vec![
        (
            Some(Method::Get),
            "/artist/lookup",
            ok(&listed(&artist(""))),
        ),
        (Some(Method::Post), "/artist", Answer::reply(400, "[]")),
    ]);

    let album = curator(&unread)
        .request(&asking(MusicUnit::Album, ABBEY_ROAD, false))
        .await;
    let artist = curator(&turned_down)
        .request(&asking(MusicUnit::Artist, BEATLES, false))
        .await;

    assert_eq!(refused(album.err()), "the albums could not be read");
    assert!(artist.is_err());
}
