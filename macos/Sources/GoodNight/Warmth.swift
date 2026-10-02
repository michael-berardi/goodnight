import SwiftUI

/// Colour numbers come from the engine (core/src/warmth.rs), the same code Windows runs.
/// This file only turns them into SwiftUI values, with a small cache because views ask often.
@MainActor
enum Warmth {
    private static var cache: [Int: Color] = [:]

    /// How white looks once the tint is on, for drawing the UI.
    static func color(_ level: Double) -> Color {
        let key = Int((clamp(level) * 1000).rounded())
        if let hit = cache[key] { return hit }
        let color: Color
        if case let .warmth(_, _, _, _, tint)? = try? Night.shared.engine.query(.warmth(level: Double(key) / 1000)), tint.count == 3 {
            color = Color(red: tint[0], green: tint[1], blue: tint[2])
        } else {
            NSLog("GoodNight: the engine did not answer a warmth query for level \(level)")
            color = .white
        }
        cache[key] = color
        return color
    }

    static var trackStops: [Gradient.Stop] {
        (0...8).map { i in Gradient.Stop(color: color(Double(i) / 8), location: Double(i) / 8) }
    }

    static func kelvinLabel(_ level: Double) -> Int {
        if case let .warmth(_, label, _, _, _)? = try? Night.shared.engine.query(.warmth(level: level)) { return label }
        return 0
    }
}

struct Preset: Identifiable, Equatable {
    let name: String, symbol: String, level: Double
    var id: String { name }

    private static let symbols = ["Day": "sun.max", "Golden": "sun.haze", "Sunset": "sun.horizon",
                                  "Candle": "flame", "Night": "moon", "Midnight": "moon.stars"]

    /// The presets the engine defines, with their SF Symbols.
    @MainActor static var all: [Preset] {
        Night.shared.engine.state.presets.map { Preset(name: $0.name, symbol: symbols[$0.name] ?? "circle", level: $0.level) }
    }
}

// Easing helpers for drawing and transitions (not engine logic).

func clamp(_ x: Double, _ lo: Double = 0, _ hi: Double = 1) -> Double { min(hi, max(lo, x)) }

func smoothstep(_ a: Double, _ b: Double, _ x: Double) -> Double {
    let t = clamp((x - a) / (b - a))
    return t * t * (3 - 2 * t)
}
