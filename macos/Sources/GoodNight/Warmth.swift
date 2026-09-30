import SwiftUI

/// One number drives everything: `level` 0 (no tint) … 1 (deep night).
/// Level maps log-linearly from 6500 K to 1200 K; the last stretch also dims.
enum Warmth {
    static let maxK = 6500.0, minK = 1200.0
    /// Lowest blue gain. About 1% of blue light (~0.15 encoded), enough that blue
    /// text, links and icons stay legible instead of turning black.
    static let blueFloor = 0.15

    static func kelvin(_ level: Double) -> Double { maxK * pow(minK / maxK, level) }

    /// Highlight level at full night. Shadows are kept; see `Display.knee`.
    static func dim(_ level: Double) -> Double { 1 - 0.42 * smoothstep(0.7, 1, level) }

    /// Per-channel gains for the display's colour table (gamma-encoded, like the table itself).
    /// Level 0 is exactly (1, 1, 1).
    static func gains(_ level: Double) -> (r: Double, g: Double, b: Double) {
        let c = whitePoint(kelvin(level)), w = whitePoint(maxK)
        var r = c.r / w.r, g = c.g / w.g, b = c.b / w.b
        let m = max(r, g, b)
        r /= m; g /= m; b /= m
        let e = 1 / 2.2
        return (pow(r, e), pow(g, e), max(blueFloor, pow(b, e)))
    }

    /// How white looks once the tint is on, for the UI.
    static func color(_ level: Double) -> Color {
        let g = gains(level), d = dim(level)
        return Color(red: g.r * d, green: g.g * d, blue: g.b * d)
    }

    static var trackStops: [Gradient.Stop] {
        (0...8).map { i in Gradient.Stop(color: color(Double(i) / 8), location: Double(i) / 8) }
    }

    /// Planckian (blackbody) white in linear sRGB. Krystek's 1985 fit of the locus in CIE 1960 uv,
    /// accurate from 1000 K to 15000 K.
    static func whitePoint(_ t: Double) -> (r: Double, g: Double, b: Double) {
        let u = (0.860117757 + 1.54118254e-4 * t + 1.28641212e-7 * t * t) / (1 + 8.42420235e-4 * t + 7.08145163e-7 * t * t)
        let v = (0.317398726 + 4.22806245e-5 * t + 4.20481691e-8 * t * t) / (1 - 2.89741816e-5 * t + 1.61456053e-7 * t * t)
        let d = 2 * u - 8 * v + 4, x = 3 * u / d, y = 2 * v / d
        let X = x / y, Z = (1 - x - y) / y
        return (max(0, 3.2406 * X - 1.5372 - 0.4986 * Z),
                max(0, -0.9689 * X + 1.8758 + 0.0415 * Z),
                max(0, 0.0557 * X - 0.2040 + 1.0570 * Z))
    }
}

struct Preset: Identifiable, Equatable {
    let name: String, symbol: String, level: Double
    var id: String { name }

    static let all = [
        Preset(name: "Day", symbol: "sun.max", level: 0),
        Preset(name: "Golden", symbol: "sun.haze", level: 0.25),
        Preset(name: "Sunset", symbol: "sun.horizon", level: 0.45),
        Preset(name: "Candle", symbol: "flame", level: 0.68),
        Preset(name: "Night", symbol: "moon", level: 0.85),
        Preset(name: "Midnight", symbol: "moon.stars", level: 1),
    ]

    static func nearest(_ level: Double) -> Preset {
        all.min { abs($0.level - level) < abs($1.level - level) }!
    }
}

func clamp(_ x: Double, _ lo: Double = 0, _ hi: Double = 1) -> Double { min(hi, max(lo, x)) }

func smoothstep(_ a: Double, _ b: Double, _ x: Double) -> Double {
    let t = clamp((x - a) / (b - a))
    return t * t * (3 - 2 * t)
}
