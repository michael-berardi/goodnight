import Foundation

/// The text shown when a newer release is available. Sparkle supplies the versions and the
/// signed release notes; these are pure so they can be tested without launching the app.
public enum UpdateNotice {
    public static func text(app: String, new: String, current: String) -> String {
        "\(app) \(new) is available — you have \(current)."
    }

    /// The one-line reminder in the menu bar popover and the window.
    public static func reminder(app: String, new: String) -> String {
        "\(app) \(new) is available"
    }
}

/// Stable `x.y.z` releases only. Prereleases and unparseable names are never newer.
public enum ReleaseVersion {
    public static func parts(_ s: String) -> [Int]? {
        let fields = s.split(separator: ".", omittingEmptySubsequences: false)
        guard fields.count == 3, fields.allSatisfy({ !$0.isEmpty && $0.allSatisfy { $0.isASCII && $0.isNumber } }) else { return nil }
        let numbers = fields.compactMap { Int($0) }
        return numbers.count == 3 ? numbers : nil
    }

    public static func isNewer(_ candidate: String, than current: String) -> Bool {
        guard let a = parts(candidate), let b = parts(current) else { return false }
        return b.lexicographicallyPrecedes(a)
    }
}

public enum UpdatePolicy {
    /// The only identifying header of an update request (Sparkle's default names the app version).
    public static let userAgent = "goodnight-update-check"
    /// The same on every machine: no locale.
    public static let headers = ["Accept-Language": "en"]

    /// Background checks are off under CI or with `GOODNIGHT_NO_UPDATE_CHECK=1`; Check Now still works.
    public static func backgroundChecksAllowed(environment: [String: String]) -> Bool {
        func set(_ key: String) -> Bool {
            guard let v = environment[key], !v.isEmpty else { return false }
            return v != "0" && v.lowercased() != "false"
        }
        return !set("GOODNIGHT_NO_UPDATE_CHECK") && !set("CI")
    }

    /// Without a Dock icon (menu bar only) a scheduled check must not open a dialog over other
    /// work: Sparkle then uses a gentle reminder, shown as a row in the menu bar popover.
    public static func standardDriverHandlesScheduledUpdate(showInDock: Bool) -> Bool { showInDock }
}
