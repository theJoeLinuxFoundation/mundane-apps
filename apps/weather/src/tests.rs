use super::View;
use gpui::TestAppContext;
use mundane_services::weather::{Location, Weather, local};
#[path = "../../../tests/providers/server.rs"]
mod fixture;
#[test]
fn conditional_cache_units_timezone_and_locations() {
    let f = fixture::Server::new();
    let service = Weather {
        endpoint: format!("{}/forecast", f.address),
        data: f.dir.path().join("data"),
        dir: f.dir.path().join("cache"),
    };
    let city = Location::custom("40.71289 -74.00601 America/New_York").unwrap();
    let first = service.fetch(&city).unwrap();
    let h = &first.forecast.hours[0];
    assert_eq!(h.temperature, 4.5);
    assert_eq!(h.wind, Some(3.2));
    assert_eq!(local(h.time, &city.zone).unwrap(), "Sun 09 Mar 03:00 EDT");
    assert!(service.fetch(&city).unwrap().status.contains("checked"));
    assert!(service.fetch(&city).unwrap().status.contains("current"));
    service
        .fetch(&Location::custom("0 0 UTC").unwrap())
        .unwrap();
    let requests = std::fs::read_to_string(f.dir.path().join("requests")).unwrap();
    let lines = requests.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].contains("lat=40.7128&lon=-74.0060"));
    assert!(lines[1].contains("Sun, 09 Mar 2025 06:00:00 GMT"));
    assert!(lines[2].contains("null"));
    assert!(
        lines
            .iter()
            .all(|l| l.contains(&mundane_services::provider::agent("weather")))
    );
}
#[gpui::test]
fn select_and_reopen_offline(cx: &mut TestAppContext) {
    let mut f = fixture::Server::new();
    let service = Weather {
        endpoint: format!("{}/forecast", f.address),
        data: f.dir.path().join("data"),
        dir: f.dir.path().join("cache"),
    };
    let (v, ui) = cx.add_window_view(|w, cx| View::new(service.clone(), w, cx));
    ui.simulate_keystrokes("o s l o enter");
    ui.run_until_parked();
    assert!(v.read_with(ui, |v, _| v.report.is_some()
        && v.location.as_ref().unwrap().name == "Oslo"));
    f.stop();
    let (v, ui) = cx.add_window_view(|w, cx| View::new(service, w, cx));
    ui.run_until_parked();
    assert!(
        v.read_with(ui, |v, _| v.status == "Offline · stale cached forecast"
            && v.list.rows.len() == 6)
    );
}

#[test]
fn daily_groups_by_local_date_and_saved_custom_places_survive_restart() {
    use mundane_services::weather::{Hour, daily};
    let hours =
        [(1741494600, 3.), (1741500000, 9.), (1741503600, 11.)].map(|(time, temperature)| Hour {
            time,
            temperature,
            condition: "cloudy".into(),
            wind: None,
        });
    let days = daily(&hours, "America/New_York").unwrap();
    assert_eq!(days.len(), 2);
    assert_eq!((days[1].low, days[1].high), (9., 11.));
    let f = fixture::Server::new();
    let s = Weather {
        data: f.dir.path().join("data"),
        dir: f.dir.path().join("cache"),
        endpoint: f.address.clone(),
    };
    let a = Location::custom("1 1 UTC").unwrap();
    let b = Location::custom("2 2 UTC").unwrap();
    for l in [&a, &b, &a] {
        s.select(l).unwrap();
    }
    assert_eq!(s.saved().unwrap(), vec![a, b]);
}
