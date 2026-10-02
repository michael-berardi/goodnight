//! The macOS Swift engine shipped in Good Night 0.1.0 is the reference. `golden-swift.txt` was
//! produced by running that engine (Sun.swift and Warmth.swift, unmodified) in eight time
//! zones on four dates. The shared core must reproduce it.

use goodnight_core::{sun, warmth};

/// UTC offset (seconds) of each zone on each golden date: 2026-03-20, 06-21, 09-23, 12-21.
fn offsets(zone: &str) -> [i32; 4] {
    match zone {
        "America/New_York" => [-14400, -14400, -14400, -18000],
        "America/Argentina/Buenos_Aires" => [-10800; 4],
        "Europe/London" => [0, 3600, 3600, 0],
        "Asia/Tokyo" => [32400; 4],
        "Australia/Sydney" => [39600, 36000, 36000, 39600],
        "Pacific/Auckland" => [46800, 43200, 43200, 46800],
        "Arctic/Longyearbyen" => [3600, 7200, 7200, 3600],
        "Asia/Kolkata" => [19800; 4],
        other => panic!("no offsets for {other}"),
    }
}

/// Unix time of 15:00 local on a civil date, matching the Swift harness.
fn local_3pm(date: &str, offset: i32) -> f64 {
    let p: Vec<i64> = date.split('-').map(|x| x.parse().unwrap()).collect();
    // days from civil (Howard Hinnant)
    let (y, m, d) = (if p[1] <= 2 { p[0] - 1 } else { p[0] }, p[1], p[2]);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    (days * 86400 + 15 * 3600) as f64 - f64::from(offset)
}

#[test]
fn sun_and_nightness_match_the_swift_engine() {
    let golden = include_str!("golden-swift.txt");
    let (mut zone, mut place, mut day_index, mut checked) = (String::new(), None, 0, 0);
    for line in golden.lines() {
        if let Some(rest) = line.strip_prefix("zone ") {
            zone = rest.split(' ').next().unwrap().to_string();
            let name = rest
                .split("place ")
                .nth(1)
                .unwrap()
                .splitn(3, ' ')
                .nth(2)
                .unwrap();
            let p = sun::place(&zone, offsets(&zone)[0]);
            assert_eq!(p.name, name, "place name for {zone}");
            place = Some(p);
            day_index = 0;
        } else if let Some(rest) = line.strip_prefix("day ") {
            let f: Vec<&str> = rest.split(' ').collect();
            let (date, rise, set) = (
                f[0],
                f[2].parse::<f64>().unwrap(),
                f[4].parse::<f64>().unwrap(),
            );
            let off = offsets(&zone)[day_index];
            day_index += 1;
            let p = place.as_ref().unwrap();
            let (r, s) = sun::events(p, local_3pm(date, off), off);
            // The Swift harness truncated to whole seconds.
            assert!(
                (r - rise).abs() < 1.0 && (s - set).abs() < 1.0,
                "{zone} {date}: rise {r} vs {rise}, set {s} vs {set}"
            );
            // Probe at the engine's own sunrise and sunset: the golden file holds them truncated to whole seconds.
            let probes = [
                r - 3600.0,
                r,
                r + 1800.0,
                (r + s) / 2.0,
                s - 1800.0,
                s,
                s + 3600.0,
                s + 10800.0,
            ];
            for (i, t) in probes.iter().enumerate() {
                let want: f64 = f[6 + i].parse().unwrap();
                let got = sun::nightness(p, *t, off);
                assert!(
                    (got - want).abs() < 2e-4,
                    "{zone} {date} probe {i}: nightness {got} vs {want}"
                );
                checked += 1;
            }
        }
    }
    assert!(checked >= 8 * 4 * 8, "only {checked} nightness probes ran");
}

#[test]
fn warmth_curve_matches_the_swift_engine() {
    let golden = include_str!("golden-swift.txt");
    let mut rows = 0;
    for line in golden.lines().filter(|l| l.starts_with("level ")) {
        let f: Vec<&str> = line.split(' ').collect();
        let level: f64 = f[1].parse().unwrap();
        let (k, d) = (f[3].parse::<f64>().unwrap(), f[5].parse::<f64>().unwrap());
        let want: Vec<f64> = f[7..10].iter().map(|x| x.parse().unwrap()).collect();
        assert!(
            (warmth::kelvin(level) - k).abs() < 1e-3,
            "kelvin at {level}"
        );
        assert!((warmth::dim(level) - d).abs() < 1e-6, "dim at {level}");
        for (i, w) in want.iter().enumerate() {
            assert!(
                (warmth::gains(level)[i] - w).abs() < 1e-6,
                "gain {i} at {level}"
            );
        }
        rows += 1;
    }
    assert_eq!(rows, 8 * 21, "expected 21 levels for each of 8 zones");
}

#[test]
fn polar_day_and_night_fall_back_to_seven_and_nineteen() {
    let p = sun::place("Arctic/Longyearbyen", 7200);
    let now = local_3pm("2026-06-21", 7200);
    let (rise, set) = sun::events(&p, now, 7200);
    let midnight = now - 15.0 * 3600.0;
    assert_eq!(
        (rise - midnight, set - midnight),
        (7.0 * 3600.0, 19.0 * 3600.0)
    );
}

#[test]
fn unknown_zone_estimates_longitude_from_the_utc_offset() {
    let p = sun::place("Nowhere/Atlantis", 3600);
    assert_eq!(p.name, "Estimated from your UTC offset");
    assert!((p.lon - 15.0).abs() < 1e-9);
}
