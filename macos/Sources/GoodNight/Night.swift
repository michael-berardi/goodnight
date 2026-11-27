import AppKit
import CarapaceFFI
import CarapaceKit
import Combine
import ServiceManagement
import SwiftUI

typealias Engine = Store<GoodNightEngine>

/// The macOS shell around the shared engine (core/). The engine decides what the settings mean,
/// which warmth to show and when; this class owns what only macOS can do: the colour tables,
/// the 1 s eased transition, wake and display changes, the login item and the Dock policy.
@MainActor
final class Night: ObservableObject {
    static let shared = Night()
    private let defaults = UserDefaults.standard

    let engine: Engine
    @Published var loginError: String?

    static let minBrightness = 0.0

    private var shown = (level: 0.0, bright: 1.0)
    private var started = false
    private var transition: Timer?, pending: DispatchWorkItem?
    private var forwarding: AnyCancellable?
    var openMain: () -> Void = {}

    private init() {
        defaults.register(defaults: ["enabled": true, "followSun": true, "manualLevel": 0.45, "dayLevel": 0.0,
                                     "nightLevel": 0.68, "brightness": 1.0, "showInMenuBar": true, "showInDock": true])
        if !defaults.bool(forKey: "showInMenuBar") && !defaults.bool(forKey: "showInDock") { defaults.set(true, forKey: "showInMenuBar") }
        let settings = GoodNightEngine.Settings(
            enabled: defaults.bool(forKey: "enabled"), followSun: defaults.bool(forKey: "followSun"),
            manualLevel: defaults.double(forKey: "manualLevel"), dayLevel: defaults.double(forKey: "dayLevel"),
            nightLevel: defaults.double(forKey: "nightLevel"), brightness: defaults.double(forKey: "brightness"))
        let zone = TimeZone.current
        do {
            engine = try Engine(
                backend: RustBackend(),
                config: .init(settings: settings, timeZone: zone.identifier, utcOffsetSeconds: zone.secondsFromGMT()))
        } catch {
            // The engine ships inside the app; failing to start it is a broken install, so say so loudly.
            NSLog("GoodNight: the engine failed to start: \(error.localizedDescription)")
            fatalError("Good Night's engine failed to start: \(error.localizedDescription)")
        }
        // Views observe Night; Night re-publishes whenever the engine's state changes.
        forwarding = engine.objectWillChange.sink { [weak self] _ in self?.objectWillChange.send() }
        engine.onEvent = { [weak self] in self?.handle($0) }
        engine.onFault = { NSLog("GoodNight: engine fault: \($0)") }
    }

    // MARK: Read

    private var state: GoodNightEngine.State { engine.state }
    var enabled: Bool { state.enabled }
    var followSun: Bool { state.followSun }
    var level: Double { state.level }
    var brightness: Double { state.brightness }
    var kelvin: Int { state.kelvin }
    var effectiveLevel: Double { enabled ? level : 0 }
    var placeName: String { state.placeName }
    var sunrise: Date { Date(timeIntervalSince1970: state.sunrise / 1000) }
    var sunset: Date { Date(timeIntervalSince1970: state.sunset / 1000) }
    var title: String { enabled ? "\(kelvin.formatted())K" : "Off" }
    var subtitle: String {
        guard enabled else { return "Your screen is untouched" }
        return brightness < 0.995 ? "\(state.preset) · \(Int((brightness * 100).rounded()))% brightness" : state.preset
    }

    /// Plain description of the next change, e.g. "Warms at 7:12 PM".
    var sunCaption: String {
        let f = DateFormatter()
        f.timeStyle = .short
        return "\(state.nextWarms ? "Warms" : "Cools") at \(f.string(from: Date(timeIntervalSince1970: state.nextAt / 1000)))"
    }

    // MARK: Lifecycle

    func start() {
        Display.capture()
        CGDisplayRegisterReconfigurationCallback({ _, flags, _ in
            if !flags.contains(.beginConfigurationFlag) { DispatchQueue.main.async { MainActor.assumeIsolated { Night.shared.displaysChanged() } } }
        }, nil)
        let ws = NSWorkspace.shared.notificationCenter
        for name in [NSWorkspace.didWakeNotification, NSWorkspace.screensDidWakeNotification, NSWorkspace.sessionDidBecomeActiveNotification] {
            ws.addObserver(forName: name, object: nil, queue: .main) { _ in MainActor.assumeIsolated { Night.shared.wake() } }
        }
        NotificationCenter.default.addObserver(forName: .NSSystemTimeZoneDidChange, object: nil, queue: .main) { _ in
            MainActor.assumeIsolated { Night.shared.timeZoneChanged() }
        }
        started = true
        render(animated: true)
    }

    func stop() {
        transition?.invalidate()
        Display.restore()
    }

    // MARK: Intent (each forwards one action to the engine and applies the result before returning,
    // so callers can wrap it in withAnimation)

    func setLevel(_ l: Double, animated: Bool) { engine.sendSync(.setLevel(level: l, animated: animated)) }
    func setBrightness(_ b: Double, animated: Bool) { engine.sendSync(.setBrightness(value: b, animated: animated)) }
    func setEnabled(_ on: Bool) { engine.sendSync(.setEnabled(on: on)) }
    func setFollowSun(_ on: Bool) { engine.sendSync(.setFollowSun(on: on)) }

    func nudge(level dl: Double = 0, brightness db: Double = 0) {
        withAnimation(.easeInOut(duration: 0.25)) { engine.sendSync(.nudge(level: dl, brightness: db)) }
    }

    // MARK: Engine events

    private func handle(_ event: GoodNightEngine.Event) {
        switch event {
        case let .render(animated):
            if started { render(animated: animated) }
        case let .persist(s):
            defaults.set(s.enabled, forKey: "enabled"); defaults.set(s.followSun, forKey: "followSun")
            defaults.set(s.manualLevel, forKey: "manualLevel"); defaults.set(s.dayLevel, forKey: "dayLevel")
            defaults.set(s.nightLevel, forKey: "nightLevel"); defaults.set(s.brightness, forKey: "brightness")
        }
    }

    // MARK: Presence — Dock, menu bar or both (stored as "showInDock" / "showInMenuBar", one always on).

    var showInDock: Bool { defaults.bool(forKey: "showInDock") }

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

    // MARK: Screen

    private func render(animated: Bool) {
        let t = state.target
        let target = (level: t.level, bright: t.brightness)
        transition?.invalidate()
        guard animated, abs(target.level - shown.level) + abs(target.bright - shown.bright) > 0.001 else {
            shown = target
            push()
            return
        }
        let from = shown, start = CACurrentMediaTime(), duration = 1.0
        let timer = Timer(timeInterval: 1.0 / 60, repeats: true) { [weak self] timer in
            MainActor.assumeIsolated {
                guard let self else { return timer.invalidate() }
                let p = min(1, (CACurrentMediaTime() - start) / duration), e = smoothstep(0, 1, p)
                self.shown = (from.level + (target.level - from.level) * e, from.bright + (target.bright - from.bright) * e)
                self.push()
                if p >= 1 { timer.invalidate() }
            }
        }
        RunLoop.main.add(timer, forMode: .common)
        transition = timer
    }

    private func push() {
        let (level, bright) = shown
        Display.apply { r, g, b in
            guard case let .ramp(r, g, b)? = try? self.engine.query(.ramp(level: level, brightness: bright, r: r, g: g, b: b)) else { return nil }
            return (r, g, b)
        }
    }

    private func displaysChanged() {
        pending?.cancel()
        let work = DispatchWorkItem { MainActor.assumeIsolated { Display.capture(); Night.shared.push() } }
        pending = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.4, execute: work)
    }

    /// macOS can reset colour tables shortly after wake; apply again once things settle.
    private func wake() {
        engine.sendSync(.refresh)
        for delay in [0.5, 2.5] {
            DispatchQueue.main.asyncAfter(deadline: .now() + delay) { MainActor.assumeIsolated { Night.shared.push() } }
        }
    }

    private func timeZoneChanged() {
        let zone = TimeZone.current
        engine.sendSync(.setTimeZone(zone: zone.identifier, utcOffsetSeconds: zone.secondsFromGMT()))
    }
}
