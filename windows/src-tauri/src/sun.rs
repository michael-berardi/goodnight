//! Sunrise and sunset without asking for location: coordinates come from the
//! time zone's reference city in the IANA zone.tab (public domain), embedded here.

use chrono::{Duration, Local, NaiveDate, TimeZone, Timelike};

use crate::warmth::smoothstep;

const ZONE_TAB: &str = include_str!("zone.tab");

pub struct Place {
    pub lat: f64,
    pub lon: f64,
    pub name: String,
}

pub fn place() -> Place {
    let zone = iana_time_zone::get_timezone().unwrap_or_default();
    let city = zone.rsplit('/').next().unwrap_or("Unknown").replace('_', " ");
    for line in ZONE_TAB.lines().filter(|l| !l.starts_with('#')) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() >= 3 && f[2] == zone {
            if let Some((lat, lon)) = parse(f[1]) {
                return Place { lat, lon, name: format!("{city}, from your time zone") };
            }
        }
    }
    eprintln!("goodnight: time zone {zone:?} not in zone.tab; estimating longitude from UTC offset");
    let offset = Local::now().offset().local_minus_utc() as f64;
    Place { lat: 35.0, lon: offset / 240.0, name: "Estimated from your UTC offset".into() }
}

/// "+4043-07400" or "-3436-05827", optionally with seconds.
fn parse(s: &str) -> Option<(f64, f64)> {
    let split = s[1..].find(['+', '-'])? + 1;
    fn deg(p: &str, d: usize) -> Option<f64> {
        let sign = if p.starts_with('-') { -1.0 } else { 1.0 };
        let digits = &p[1..];
        let dd: f64 = digits.get(..d)?.parse().ok()?;
        let mm: f64 = digits.get(d..d + 2)?.parse().ok()?;
        let ss: f64 = digits.get(d + 2..d + 4).and_then(|x| x.parse().ok()).unwrap_or(0.0);
        Some(sign * (dd + mm / 60.0 + ss / 3600.0))
    }
    Some((deg(&s[..split], 2)?, deg(&s[split..], 3)?))
}

/// Sunrise and sunset (unix seconds) for a local calendar day.
pub fn events(place: &Place, day: NaiveDate) -> (f64, f64) {
    let noon = Local
        .from_local_datetime(&day.and_hms_opt(12, 0, 0).unwrap())
        .earliest()
        .map(|t| t.timestamp() as f64)
        .unwrap_or(0.0);
    let rad = std::f64::consts::PI / 180.0;
    let jd = noon / 86400.0 + 2440587.5;
    let n = (jd - 2451545.0 + 0.0008).round();
    let j_star = n - place.lon / 360.0;
    let m = (357.5291 + 0.98560028 * j_star).rem_euclid(360.0);
    let c = 1.9148 * (m * rad).sin() + 0.02 * (2.0 * m * rad).sin() + 0.0003 * (3.0 * m * rad).sin();
    let lambda = (m + c + 180.0 + 102.9372).rem_euclid(360.0);
    let transit = 2451545.0 + j_star + 0.0053 * (m * rad).sin() - 0.0069 * (2.0 * lambda * rad).sin();
    let dec = ((lambda * rad).sin() * (23.44 * rad).sin()).asin();
    let cos_w = ((-0.833 * rad).sin() - (place.lat * rad).sin() * dec.sin()) / ((place.lat * rad).cos() * dec.cos());
    let to_unix = |j: f64| (j - 2440587.5) * 86400.0;
    if cos_w.abs() > 1.0 {
        // Polar day or night: 07:00 and 19:00.
        return (noon - 5.0 * 3600.0, noon + 7.0 * 3600.0);
    }
    let w = cos_w.acos() / rad;
    (to_unix(transit - w / 360.0), to_unix(transit + w / 360.0))
}

/// 0 in daylight, 1 at night. Warms over an hour around sunset, cools over 45 minutes around sunrise.
pub fn nightness(place: &Place) -> f64 {
    let now = Local::now();
    let (rise, set) = events(place, now.date_naive());
    let t = now.timestamp() as f64;
    if t < (rise + set) / 2.0 {
        1.0 - smoothstep(rise - 30.0 * 60.0, rise + 15.0 * 60.0, t)
    } else {
        smoothstep(set - 20.0 * 60.0, set + 40.0 * 60.0, t)
    }
}

/// The next change: (warms, unix seconds).
pub fn next_change(place: &Place) -> (bool, f64) {
    let now = Local::now();
    let (rise, set) = events(place, now.date_naive());
    let t = now.timestamp() as f64;
    if t < rise {
        (false, rise)
    } else if t < set {
        (true, set)
    } else {
        let tomorrow = (now + Duration::days(1)).with_hour(12).unwrap_or(now).date_naive();
        (false, events(place, tomorrow).0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_zone_tab_coordinates() {
        let (lat, lon) = parse("-3436-05827").unwrap();
        assert!((lat + 34.6).abs() < 0.01 && (lon + 58.45).abs() < 0.01);
        let (lat, lon) = parse("+404251-0740023").unwrap();
        assert!((lat - 40.714).abs() < 0.01 && (lon + 74.006).abs() < 0.01);
    }

    #[test]
    fn day_is_about_twelve_hours_at_equinox() {
        let p = Place { lat: 40.71, lon: -74.01, name: String::new() };
        let (rise, set) = events(&p, NaiveDate::from_ymd_opt(2026, 9, 23).unwrap());
        let hours = (set - rise) / 3600.0;
        assert!((hours - 12.1).abs() < 0.3, "{hours}");
    }
}
