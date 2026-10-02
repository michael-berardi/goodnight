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

    /// Applies a tint to every display. `ramp` receives a display's own calibration curve
    /// (three tables, 0…1) and returns the table to set; the colour pipeline itself lives in the
    /// engine, shared with Windows. Returns false if any display could not be set.
    @discardableResult
    static func apply(_ ramp: (_ r: [Double], _ g: [Double], _ b: [Double]) -> (r: [Double], g: [Double], b: [Double])?) -> Bool {
        var ok = true
        for (id, t) in base {
            guard let out = ramp(t.r.map(Double.init), t.g.map(Double.init), t.b.map(Double.init)),
                  out.r.count == t.r.count, out.g.count == t.g.count, out.b.count == t.b.count else {
                NSLog("GoodNight: the engine returned no usable ramp for display \(id)")
                ok = false
                continue
            }
            let r = out.r.map(CGGammaValue.init), g = out.g.map(CGGammaValue.init), b = out.b.map(CGGammaValue.init)
            let err = CGSetDisplayTransferByTable(id, UInt32(r.count), r, g, b)
            if err != .success { NSLog("GoodNight: cannot set colour table for display \(id) (CGError \(err.rawValue))"); ok = false }
        }
        return ok
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
