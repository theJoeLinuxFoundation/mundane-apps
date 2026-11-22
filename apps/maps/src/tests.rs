use super::{
    Maps,
    atlas::{Atlas, Viewport},
    geo::{Camera, Size, Tile},
    source::{Area, MAX_BYTES, MAX_TILES, Source},
    vector,
};
use gpui::TestAppContext;
#[path = "../../../tests/providers/server.rs"]
mod fixture;
fn source(f: &fixture::Server) -> Source {
    Source {
        dir: f.dir.path().join("maps"),
        tiles: format!("{}/tilejson", f.address),
        search: format!("{}/search", f.address),
    }
}
fn viewport() -> Viewport {
    Viewport {
        camera: Camera {
            lat: 0.,
            lon: 0.,
            zoom: 1,
        },
        size: Size {
            width: 256,
            height: 256,
            scale: 100,
        },
        place: None,
        offline: None,
        contrast: false,
    }
}
#[test]
fn missing_tiles_and_area_limits() {
    let f = fixture::Server::new();
    let s = source(&f);
    std::fs::write(f.dir.path().join("missing"), "").unwrap();
    let scene = Atlas::default().render(&s, &viewport(), || false).unwrap();
    assert_eq!(scene.missing, 4);
    assert!(scene.area.is_none());
    let camera = Camera {
        zoom: 12,
        ..viewport().camera
    };
    let id = Tile { x: 0, y: 0, z: 12 };
    let mut a = Area {
        camera,
        place: None,
        tiles: (0..=MAX_TILES)
            .map(|x| (Tile { x: x as u32, ..id }, vec![1].into()))
            .collect(),
    };
    assert!(s.save(&a).is_err());
    a.tiles.truncate(MAX_TILES);
    a.check().unwrap();
    a.tiles = (0..13)
        .map(|x| (Tile { x, ..id }, vec![0u8; 4_000_000].into()))
        .collect();
    assert!(a.tiles.iter().map(|p| p.1.len()).sum::<usize>() > MAX_BYTES);
    assert!(s.save(&a).is_err());
    assert!(!s.has_area());
    assert!(vector::paint(&[255], 256, false).is_err());
}
#[test]
fn place_cache_and_rate_across_clients() {
    let mut f = fixture::Server::new();
    let s = source(&f);
    let copy = s.clone();
    std::thread::scope(|scope| {
        let one = scope.spawn(|| s.places("north coast", false).unwrap());
        let two = scope.spawn(|| copy.places("east coast", false).unwrap());
        assert_eq!(one.join().unwrap().len(), 2);
        assert_eq!(two.join().unwrap().len(), 2)
    });
    f.stop();
    assert_eq!(copy.places("  NORTH   coast ", true).unwrap().len(), 2);
    assert!(s.places("uncached", true).is_err());
    let log = std::fs::read_to_string(f.dir.path().join("requests")).unwrap();
    let lines = log
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 2);
    assert!(
        lines
            .iter()
            .all(|l| l[1].as_str().unwrap() == mundane_services::provider::agent("maps"))
    );
    assert!(lines[1][3].as_f64().unwrap() - lines[0][3].as_f64().unwrap() >= 1.);
}
#[gpui::test]
fn select_pan_zoom_save_and_reopen(cx: &mut TestAppContext) {
    let mut f = fixture::Server::new();
    let s = source(&f);
    let (v, ui) = cx.add_window_view(|w, cx| Maps::new(s.clone(), w, cx));
    ui.run_until_parked();
    ui.simulate_keystrokes("ctrl-f q u a y");
    ui.run_until_parked();
    let before = std::fs::read_to_string(f.dir.path().join("requests")).unwrap_or_default();
    assert!(!before.contains("/search?"));
    ui.simulate_keystrokes("enter");
    ui.run_until_parked();
    ui.simulate_keystrokes("down enter");
    ui.run_until_parked();
    assert!(v.read_with(ui, |v, _| v.place.as_ref().unwrap().name()
        == "Harbour Point"));
    ui.simulate_keystrokes("right =");
    ui.run_until_parked();
    v.read_with(ui, |v, _| {
        assert_eq!(v.camera.zoom, 13);
        assert!(v.camera.lon > 10.7522);
        assert!(v.ready, "{}", v.status);
        assert_eq!(v.scene.as_ref().unwrap().missing, 0)
    });
    ui.simulate_keystrokes("ctrl-s");
    ui.run_until_parked();
    assert!(s.has_area());
    f.stop();
    let (v, ui) = cx.add_window_view(|w, cx| Maps::new(s, w, cx));
    ui.run_until_parked();
    assert!(v.read_with(ui, |v, _| v.offline.is_some()
        && v.status == "Saved area · offline"));
}
