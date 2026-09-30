//! One number drives everything: `level` 0 (no tint) … 1 (deep night).
//! Identical to the macOS engine (macos/Sources/GoodNight/Warmth.swift).

pub const MAX_K: f64 = 6500.0;
pub const MIN_K: f64 = 1200.0;
/// Lowest blue gain (~1% of blue light) so blue text and icons stay legible.
pub const BLUE_FLOOR: f64 = 0.15;

pub struct Preset {
    pub name: &'static str,
    pub level: f64,
}

pub const PRESETS: [Preset; 6] = [
    Preset { name: "Day", level: 0.0 },
    Preset { name: "Golden", level: 0.25 },
    Preset { name: "Sunset", level: 0.45 },
    Preset { name: "Candle", level: 0.68 },
    Preset { name: "Night", level: 0.85 },
    Preset { name: "Midnight", level: 1.0 },
];

pub fn nearest(level: f64) -> &'static Preset {
    PRESETS.iter().min_by(|a, b| (a.level - level).abs().total_cmp(&(b.level - level).abs())).unwrap()
}

pub fn clamp(x: f64, lo: f64, hi: f64) -> f64 {
    x.max(lo).min(hi)
}

pub fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = clamp((x - a) / (b - a), 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn kelvin(level: f64) -> f64 {
    MAX_K * (MIN_K / MAX_K).powf(level)
}

/// Where full white lands; shadows are kept by `knee`.
pub fn dim(level: f64) -> f64 {
    1.0 - 0.42 * smoothstep(0.7, 1.0, level)
}

/// `e · dim^e`: pulls highlights down, leaves shadows at full contrast.
pub fn knee(e: f64, dim: f64) -> f64 {
    e * dim.powf(e)
}

/// Per-channel gains (gamma-encoded). Level 0 is exactly [1, 1, 1].
pub fn gains(level: f64) -> [f64; 3] {
    let c = white_point(kelvin(level));
    let w = white_point(MAX_K);
    let mut g = [c[0] / w[0], c[1] / w[1], c[2] / w[2]];
    let m = g[0].max(g[1]).max(g[2]);
    for v in g.iter_mut() {
        *v = (*v / m).powf(1.0 / 2.2);
    }
    g[2] = g[2].max(BLUE_FLOOR);
    g
}

/// Planckian white in linear sRGB (Krystek 1985, CIE 1960 uv), 1000–15000 K.
pub fn white_point(t: f64) -> [f64; 3] {
    let u = (0.860117757 + 1.54118254e-4 * t + 1.28641212e-7 * t * t) / (1.0 + 8.42420235e-4 * t + 7.08145163e-7 * t * t);
    let v = (0.317398726 + 4.22806245e-5 * t + 4.20481691e-8 * t * t) / (1.0 - 2.89741816e-5 * t + 1.61456053e-7 * t * t);
    let d = 2.0 * u - 8.0 * v + 4.0;
    let (x, y) = (3.0 * u / d, 2.0 * v / d);
    let (cx, cz) = (x / y, (1.0 - x - y) / y);
    [
        (3.2406 * cx - 1.5372 - 0.4986 * cz).max(0.0),
        (-0.9689 * cx + 1.8758 + 0.0415 * cz).max(0.0),
        (0.0557 * cx - 0.2040 + 1.0570 * cz).max(0.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_zero_is_identity() {
        let g = gains(0.0);
        for v in g {
            assert!((v - 1.0).abs() < 1e-9);
        }
        assert_eq!(dim(0.0), 1.0);
    }

    #[test]
    fn matches_macos_probe() {
        // Values printed by `GoodNight --probe` on macOS.
        let g = gains(0.68);
        assert!((g[1] - 0.568).abs() < 0.002 && (g[2] - 0.15).abs() < 0.001);
        let g = gains(1.0);
        assert!((g[1] - 0.281).abs() < 0.002);
        assert!((knee(1.0, dim(1.0)) - 0.58).abs() < 0.001);
    }

    #[test]
    fn knee_is_monotonic() {
        let d = dim(1.0);
        let mut last = 0.0;
        for i in 0..=255 {
            let v = knee(i as f64 / 255.0, d);
            assert!(v >= last);
            last = v;
        }
    }
}
