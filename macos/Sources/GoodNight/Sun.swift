import Foundation

/// Sunrise and sunset without asking for location: the coordinates come from the
/// system time zone's reference city in /usr/share/zoneinfo/zone.tab.
enum Sun {
    static let place: (lat: Double, lon: Double) = lookup(TimeZone.current.identifier)
        ?? (35, Double(TimeZone.current.secondsFromGMT()) / 240)

    /// "New York, from your time zone".
    static var placeName: String {
        let city = TimeZone.current.identifier.split(separator: "/").last.map { $0.replacingOccurrences(of: "_", with: " ") } ?? "Unknown"
        return "\(city), from your time zone"
    }

    /// Sunrise and sunset for the calendar day containing `date` (standard sunrise equation).
    static func events(on date: Date) -> (rise: Date, set: Date) {
        let cal = Calendar.current
        let noon = cal.date(bySettingHour: 12, minute: 0, second: 0, of: date) ?? date
        let rad = Double.pi / 180
        let jd = noon.timeIntervalSince1970 / 86400 + 2440587.5
        let n = (jd - 2451545 + 0.0008).rounded()
        let jStar = n - place.lon / 360
        let m = (357.5291 + 0.98560028 * jStar).truncatingRemainder(dividingBy: 360)
        let c = 1.9148 * sin(m * rad) + 0.02 * sin(2 * m * rad) + 0.0003 * sin(3 * m * rad)
        let lambda = (m + c + 180 + 102.9372).truncatingRemainder(dividingBy: 360)
        let transit = 2451545 + jStar + 0.0053 * sin(m * rad) - 0.0069 * sin(2 * lambda * rad)
        let dec = asin(sin(lambda * rad) * sin(23.44 * rad))
        let cosW = (sin(-0.833 * rad) - sin(place.lat * rad) * sin(dec)) / (cos(place.lat * rad) * cos(dec))
        func toDate(_ j: Double) -> Date { Date(timeIntervalSince1970: (j - 2440587.5) * 86400) }
        guard abs(cosW) <= 1 else {
            // Polar day or night: fall back to 7:00 and 19:00.
            let at = { (h: Int) in cal.date(bySettingHour: h, minute: 0, second: 0, of: date) ?? date }
            return (at(7), at(19))
        }
        let w = acos(cosW) / rad
        return (toDate(transit - w / 360), toDate(transit + w / 360))
    }

    /// 0 in daylight, 1 at night. Warms over an hour around sunset, cools over 45 minutes around sunrise.
    static func nightness(at now: Date = Date()) -> Double {
        let e = events(on: now)
        let t = now.timeIntervalSince1970
        let midday = (e.rise.timeIntervalSince1970 + e.set.timeIntervalSince1970) / 2
        if t < midday {
            let r = e.rise.timeIntervalSince1970
            return 1 - smoothstep(r - 30 * 60, r + 15 * 60, t)
        }
        let s = e.set.timeIntervalSince1970
        return smoothstep(s - 20 * 60, s + 40 * 60, t)
    }

    /// Plain description of the next change, e.g. "Warms at 7:12 PM".
    static func nextChange(at now: Date = Date()) -> String {
        let e = events(on: now)
        let f = DateFormatter()
        f.timeStyle = .short
        if now < e.rise { return "Cools at \(f.string(from: e.rise))" }
        if now < e.set { return "Warms at \(f.string(from: e.set))" }
        let tomorrow = Calendar.current.date(byAdding: .day, value: 1, to: now) ?? now
        return "Cools at \(f.string(from: events(on: tomorrow).rise))"
    }

    private static func lookup(_ zone: String) -> (Double, Double)? {
        guard let tab = try? String(contentsOfFile: "/usr/share/zoneinfo/zone.tab", encoding: .utf8) else {
            NSLog("GoodNight: zone.tab unreadable; estimating longitude from UTC offset")
            return nil
        }
        for line in tab.split(separator: "\n") where !line.hasPrefix("#") {
            let f = line.split(separator: "\t")
            guard f.count >= 3, f[2] == zone else { continue }
            return parse(String(f[1]))
        }
        NSLog("GoodNight: time zone \(zone) not in zone.tab; estimating longitude from UTC offset")
        return nil
    }

    /// "+4043-07400" or "-3436-05827" or with seconds: ±DDMM[SS]±DDDMM[SS].
    private static func parse(_ s: String) -> (Double, Double)? {
        guard let split = s.dropFirst().firstIndex(where: { $0 == "+" || $0 == "-" }) else { return nil }
        func deg(_ p: Substring, _ d: Int) -> Double? {
            let sign: Double = p.first == "-" ? -1 : 1
            let digits = Array(p.dropFirst())
            guard digits.count >= d + 2, let dd = Double(String(digits[0..<d])),
                  let mm = Double(String(digits[d..<d + 2])) else { return nil }
            let ss = digits.count >= d + 4 ? Double(String(digits[(d + 2)..<(d + 4)])) ?? 0 : 0
            return sign * (dd + mm / 60 + ss / 3600)
        }
        guard let lat = deg(s[..<split], 2), let lon = deg(s[split...], 3) else { return nil }
        return (lat, lon)
    }
}
