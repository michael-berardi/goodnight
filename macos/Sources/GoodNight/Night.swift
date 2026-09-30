import AppKit
import ServiceManagement
import SwiftUI

/// App state. The screen is only touched in `render`; timers run only while something changes
/// (a 1 s transition) or once a minute when following the sun.
final class Night: ObservableObject {
    static let shared = Night()
    private let store = UserDefaults.standard

    @Published private(set) var enabled: Bool
    @Published private(set) var followSun: Bool
    @Published private(set) var level: Double
    /// Software brightness, 0 (black) … 1. Scales the same colour table, so it costs nothing.
    @Published private(set) var brightness: Double
    @Published private(set) var sunCaption = ""
    @Published var loginError: String?

    static let minBrightness = 0.0, launchBrightness = 0.3

    private var manualLevel: Double, dayLevel: Double, nightLevel: Double
    private var shown = (level: 0.0, bright: 1.0)
    private var transition: Timer?, clock: Timer?, pending: DispatchWorkItem?
    var openMain: () -> Void = {}

    private init() {
        store.register(defaults: ["enabled": true, "followSun": true, "manualLevel": 0.45, "dayLevel": 0.0,
                                  "nightLevel": 0.68, "brightness": 1.0, "showInMenuBar": true, "showInDock": true])
        enabled = store.bool(forKey: "enabled")
        followSun = store.bool(forKey: "followSun")
        manualLevel = store.double(forKey: "manualLevel")
        dayLevel = store.double(forKey: "dayLevel")
        nightLevel = store.double(forKey: "nightLevel")
        level = manualLevel
        // Never start on a screen too dark to find the controls.
        brightness = max(Night.launchBrightness, store.double(forKey: "brightness"))
        if !store.bool(forKey: "showInMenuBar") && !store.bool(forKey: "showInDock") { store.set(true, forKey: "showInMenuBar") }
    }

    var kelvin: Int { Int((Warmth.kelvin(level) / 50).rounded() * 50) }
    var effectiveLevel: Double { enabled ? level : 0 }
    var title: String { enabled ? "\(kelvin.formatted())K" : "Off" }
    var subtitle: String {
        guard enabled else { return "Your screen is untouched" }
        let name = Preset.nearest(level).name
        return brightness < 0.995 ? "\(name) · \(Int((brightness * 100).rounded()))% brightness" : name
    }

    func start() {
        Display.capture()
        CGDisplayRegisterReconfigurationCallback({ _, flags, _ in
            if !flags.contains(.beginConfigurationFlag) { DispatchQueue.main.async { Night.shared.displaysChanged() } }
        }, nil)
        let ws = NSWorkspace.shared.notificationCenter
        for name in [NSWorkspace.didWakeNotification, NSWorkspace.screensDidWakeNotification, NSWorkspace.sessionDidBecomeActiveNotification] {
            ws.addObserver(forName: name, object: nil, queue: .main) { _ in Night.shared.reapplySoon() }
        }
        if followSun { level = autoLevel }
        schedule()
        render(animated: true)
    }

    func stop() {
        transition?.invalidate()
        Display.restore()
    }

    // MARK: Intent

    func setLevel(_ l: Double, animated: Bool) {
        level = clamp(l)
        if followSun { if Sun.nightness() >= 0.5 { nightLevel = level } else { dayLevel = level } } else { manualLevel = level }
        store.set(manualLevel, forKey: "manualLevel"); store.set(dayLevel, forKey: "dayLevel"); store.set(nightLevel, forKey: "nightLevel")
        turnOn()
        render(animated: animated)
    }

    func setBrightness(_ b: Double, animated: Bool) {
        brightness = clamp(b, Night.minBrightness, 1)
        store.set(brightness, forKey: "brightness")
        turnOn()
        render(animated: animated)
    }

    func nudge(level dl: Double = 0, brightness db: Double = 0) {
        withAnimation(.easeInOut(duration: 0.25)) {
            if dl != 0 { setLevel(level + dl, animated: true) }
            if db != 0 { setBrightness(brightness + db, animated: true) }
        }
    }

    func setEnabled(_ on: Bool) {
        enabled = on
        store.set(on, forKey: "enabled")
        render(animated: true)
    }

    func setFollowSun(_ on: Bool) {
        followSun = on
        store.set(on, forKey: "followSun")
        level = on ? autoLevel : manualLevel
        schedule()
        render(animated: true)
    }

    private func turnOn() {
        if !enabled { enabled = true; store.set(true, forKey: "enabled") }
    }

    // MARK: Presence — Dock, menu bar or both (stored as "showInDock" / "showInMenuBar", one always on).

    var showInDock: Bool { store.bool(forKey: "showInDock") }

    func applyDockPolicy() {
        NSApp.setActivationPolicy(showInDock ? .regular : .accessory)
        if !showInDock { DispatchQueue.main.async { NSApp.activate(ignoringOtherApps: true) } }
    }

    var launchAtLogin: Bool { SMAppService.mainApp.status == .enabled }

    func setLaunchAtLogin(_ on: Bool) {
        do {
            if on { try SMAppService.mainApp.register() } else { try SMAppService.mainApp.unregister() }
            loginError = nil
        } catch {
            loginError = "Couldn't change the login item: \(error.localizedDescription)"
            NSLog("GoodNight: \(loginError!)")
        }
        objectWillChange.send()
    }

    // MARK: Schedule

    private var autoLevel: Double {
        dayLevel + (nightLevel - dayLevel) * Sun.nightness()
    }

    private func schedule() {
        clock?.invalidate()
        sunCaption = Sun.nextChange()
        guard followSun else { return }
        let t = Timer(timeInterval: 60, repeats: true) { _ in Night.shared.tick() }
        t.tolerance = 20
        RunLoop.main.add(t, forMode: .common)
        clock = t
    }

    private func tick() {
        sunCaption = Sun.nextChange()
        let target = autoLevel
        if abs(target - level) > 0.0005 {
            withAnimation(.easeInOut(duration: 1)) { level = target }
        }
        render(animated: true)
    }

    // MARK: Screen

    private func render(animated: Bool) {
        let target = enabled ? (level: level, bright: brightness) : (level: 0.0, bright: 1.0)
        transition?.invalidate()
        guard animated, abs(target.level - shown.level) + abs(target.bright - shown.bright) > 0.001 else {
            shown = target
            push()
            return
        }
        let from = shown, start = CACurrentMediaTime(), duration = 1.0
        let t = Timer(timeInterval: 1.0 / 60, repeats: true) { [weak self] timer in
            guard let self else { return timer.invalidate() }
            let p = min(1, (CACurrentMediaTime() - start) / duration), e = smoothstep(0, 1, p)
            self.shown = (from.level + (target.level - from.level) * e, from.bright + (target.bright - from.bright) * e)
            self.push()
            if p >= 1 { timer.invalidate() }
        }
        RunLoop.main.add(t, forMode: .common)
        transition = t
    }

    private func push() {
        Display.apply(Warmth.gains(shown.level), dim: Warmth.dim(shown.level), brightness: shown.bright)
    }

    private func displaysChanged() {
        pending?.cancel()
        let work = DispatchWorkItem { Display.capture(); Night.shared.push() }
        pending = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.4, execute: work)
    }

    /// macOS can reset colour tables shortly after wake; apply again once things settle.
    private func reapplySoon() {
        for delay in [0.5, 2.5] {
            DispatchQueue.main.asyncAfter(deadline: .now() + delay) { Night.shared.push() }
        }
    }
}
