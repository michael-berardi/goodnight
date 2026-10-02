//! Sunrise and sunset without asking for location: coordinates come from the time
//! zone's reference city in the IANA zone.tab (public domain), embedded here.
//!
//! Everything is a pure function of (place, unix time, UTC offset), so the same
//! numbers come out on every platform and tests can pin any instant.

use crate::warmth::smoothstep;

const ZONE_TAB: &str = include_str!("zone.tab");
const DAY: f64 = 86_400.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub lat: f64,
    pub lon: f64,
    pub name: String,
}

/// Where a time zone's reference city is. Unknown zones fall back to a longitude
/// estimated from the UTC offset (and the name says so).
pub fn place(zone: &str, utc_offset_seconds: i32) -> Place {
    let city = zone
        .rsplit('/')
        .next()
        .filter(|c| !c.is_empty())
        .unwrap_or("Unknown")
        .replace('_', " ");
    for line in ZONE_TAB.lines().filter(|l| !l.starts_with('#')) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() >= 3 && f[2] == zone {
            if let Some((lat, lon)) = parse(f[1]) {
                return Place {
                    lat,
                    lon,
                    name: format!("{city}, from your time zone"),
                };
            }
        }
    }
    Place {
        lat: 35.0,
        lon: f64::from(utc_offset_seconds) / 240.0,
        name: "Estimated from your UTC offset".into(),
    }
}

/// "+4043-07400" or "-3436-05827", optionally with seconds: ±DDMM[SS]±DDDMM[SS].
fn parse(s: &str) -> Option<(f64, f64)> {
    let split = s.get(1..)?.find(['+', '-'])? + 1;
    fn deg(p: &str, d: usize) -> Option<f64> {
        let sign = if p.starts_with('-') { -1.0 } else { 1.0 };
        let digits = p.get(1..)?;
        let dd: f64 = digits.get(..d)?.parse().ok()?;
        let mm: f64 = digits.get(d..d + 2)?.parse().ok()?;
        let ss: f64 = digits
            .get(d + 2..d + 4)
            .and_then(|x| x.parse().ok())
            .unwrap_or(0.0);
        Some(sign * (dd + mm / 60.0 + ss / 3600.0))
    }
    Some((deg(&s[..split], 2)?, deg(&s[split..], 3)?))
}

/// Unix time of local midnight starting the day that contains `now`.
fn day_start(now: f64, utc_offset: i32) -> f64 {
    let off = f64::from(utc_offset);
    ((now + off) / DAY).floor() * DAY - off
}

/// Sunrise and sunset (unix seconds) for the local calendar day containing `now`.
/// Standard sunrise equation. In polar day or night: 07:00 and 19:00 local.
pub fn events(place: &Place, now: f64, utc_offset: i32) -> (f64, f64) {
    let start = day_start(now, utc_offset);
    let noon = start + 12.0 * 3600.0;
    let rad = std::f64::consts::PI / 180.0;
    let jd = noon / DAY + 2440587.5;
    let n = (jd - 2451545.0 + 0.0008).round();
    let j_star = n - place.lon / 360.0;
    let m = (357.5291 + 0.98560028 * j_star).rem_euclid(360.0);
    let c =
        1.9148 * (m * rad).sin() + 0.02 * (2.0 * m * rad).sin() + 0.0003 * (3.0 * m * rad).sin();
    let lambda = (m + c + 180.0 + 102.9372).rem_euclid(360.0);
    let transit =
        2451545.0 + j_star + 0.0053 * (m * rad).sin() - 0.0069 * (2.0 * lambda * rad).sin();
    let dec = ((lambda * rad).sin() * (23.44 * rad).sin()).asin();
    let cos_w = ((-0.833 * rad).sin() - (place.lat * rad).sin() * dec.sin())
        / ((place.lat * rad).cos() * dec.cos());
    if cos_w.abs() > 1.0 {
        return (start + 7.0 * 3600.0, start + 19.0 * 3600.0);
    }
    let to_unix = |j: f64| (j - 2440587.5) * DAY;
    let w = cos_w.acos() / rad;
    (to_unix(transit - w / 360.0), to_unix(transit + w / 360.0))
}

/// 0 in daylight, 1 at night. Warms over an hour around sunset, cools over 45 minutes around sunrise.
pub fn nightness(place: &Place, now: f64, utc_offset: i32) -> f64 {
    let (rise, set) = events(place, now, utc_offset);
    if now < (rise + set) / 2.0 {
        1.0 - smoothstep(rise - 30.0 * 60.0, rise + 15.0 * 60.0, now)
    } else {
        smoothstep(set - 20.0 * 60.0, set + 40.0 * 60.0, now)
    }
}

/// The next change as (warms?, unix seconds).
pub fn next_change(place: &Place, now: f64, utc_offset: i32) -> (bool, f64) {
    let (rise, set) = events(place, now, utc_offset);
    if now < rise {
        (false, rise)
    } else if now < set {
        (true, set)
    } else {
        (false, events(place, now + DAY, utc_offset).0)
    }
}
