import CoreGraphics
import Foundation

/// Tints every display through its hardware colour lookup table (the same path f.lux uses).
/// The GPU does no extra work and nothing is drawn on screen. macOS resets the tables
/// when this process exits, so a crash can never leave the screen tinted.
enum Display {
    private struct Table { var r, g, b: [CGGammaValue] }
    private static var base: [CGDirectDisplayID: Table] = [:]

    /// Reads each display's own calibration curve so the tint multiplies it instead of replacing it.
    static func capture() {
        CGDisplayRestoreColorSyncSettings()
        base = [:]
        for id in onlineDisplays() {
            var t = Table(r: .init(repeating: 0, count: 256), g: .init(repeating: 0, count: 256), b: .init(repeating: 0, count: 256))
            var n: UInt32 = 0
            let err = CGGetDisplayTransferByTable(id, 256, &t.r, &t.g, &t.b, &n)
            guard err == .success, n > 0 else {
                NSLog("GoodNight: cannot read colour table for display \(id) (CGError \(err.rawValue)); using linear")
                base[id] = linear
                continue
            }
            t.r.removeLast(256 - Int(n)); t.g.removeLast(256 - Int(n)); t.b.removeLast(256 - Int(n))
            base[id] = t
        }
    }

    /// `dim` is where full white lands. Instead of scaling every level equally, `e · dim^e`
    /// pulls highlights down and leaves shadows at full contrast, so dark text and dark-mode
    /// UI stay readable while bright pages stop glaring.
    static func knee(_ e: CGGammaValue, _ dim: CGGammaValue) -> CGGammaValue { e * pow(dim, e) }

    /// `brightness` scales the gamma-encoded signal, which tracks perceived lightness closely,
    /// so the slider feels even from full down to black.
    static func apply(_ gain: (r: Double, g: Double, b: Double), dim: Double, brightness: Double) {
        let d = CGGammaValue(dim), k = CGGammaValue(brightness)
        let gr = CGGammaValue(gain.r) * k, gg = CGGammaValue(gain.g) * k, gb = CGGammaValue(gain.b) * k
        for (id, t) in base {
            let r = t.r.map { knee($0, d) * gr }
            let g = t.g.map { knee($0, d) * gg }
            let b = t.b.map { knee($0, d) * gb }
            let err = CGSetDisplayTransferByTable(id, UInt32(r.count), r, g, b)
            if err != .success { NSLog("GoodNight: cannot set colour table for display \(id) (CGError \(err.rawValue))") }
        }
    }

    static func restore() { CGDisplayRestoreColorSyncSettings() }

    private static var linear: Table {
        let ramp = (0..<256).map { CGGammaValue($0) / 255 }
        return Table(r: ramp, g: ramp, b: ramp)
    }

    private static func onlineDisplays() -> [CGDirectDisplayID] {
        var ids = [CGDirectDisplayID](repeating: 0, count: 16)
        var count: UInt32 = 0
        guard CGGetOnlineDisplayList(16, &ids, &count) == .success else { return [] }
        return Array(ids.prefix(Int(count)))
    }
}
