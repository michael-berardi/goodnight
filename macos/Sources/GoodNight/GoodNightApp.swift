import AppKit
import Carbon.HIToolbox
import Sparkle
import SwiftUI
import UpdateKit

@main
struct GoodNightApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    @StateObject private var night = Night.shared
    @AppStorage("showInMenuBar") private var showInMenuBar = true

    init() {
        if CommandLine.arguments.contains("--probe") { Probe.run(); exit(0) }
        if CommandLine.arguments.contains("--update-probe") { UpdateProbe.run(); exit(0) }
    }

    var body: some Scene {
        Window("Good Night", id: "main") {
            MainView().environmentObject(night)
        }
        .windowStyle(.hiddenTitleBar)
        .windowResizability(.contentSize)
        .commands { AppCommands(night: night) }

        Settings {
            SettingsView().environmentObject(night)
        }

        MenuBarExtra(isInserted: $showInMenuBar) {
            MenuView().environmentObject(night)
        } label: {
            MenuIcon().environmentObject(night)
        }
        .menuBarExtraStyle(.window)
    }
}

/// Hands SwiftUI's "open window" action to the app delegate, so the Dock and the menu bar
/// can bring the window back.
@MainActor func keepWindowOpener(_ openWindow: OpenWindowAction) {
    Night.shared.openMain = {
        openWindow(id: "main")
        NSApp.activate(ignoringOtherApps: true)
    }
}

private struct MenuIcon: View {
    @EnvironmentObject var night: Night
    @Environment(\.openWindow) private var openWindow
    var body: some View {
        Image(systemName: night.enabled && night.level > 0.01 ? "moon.fill" : "moon")
            .onAppear { keepWindowOpener(openWindow) }
    }
}

struct AppCommands: Commands {
    @ObservedObject var night: Night

    var body: some Commands {
        CommandGroup(replacing: .newItem) {
            Button("Show Good Night") { night.openMain() }.keyboardShortcut("n")
        }
        CommandMenu("Controls") {
            Button(night.enabled ? "Turn Off" : "Turn On") { withAnimation(.easeInOut(duration: 1)) { night.setEnabled(!night.enabled) } }
                .keyboardShortcut("t", modifiers: [.command, .shift])
            Divider()
            ForEach(Array(Preset.all.enumerated()), id: \.offset) { i, p in
                Button(p.name) { withAnimation(.easeInOut(duration: 1)) { night.setLevel(p.level, animated: true) } }
                    .keyboardShortcut(KeyEquivalent(Character(String(i + 1))), modifiers: .command)
            }
            Divider()
            Button("Warmer") { night.nudge(level: 0.05) }.keyboardShortcut(.rightArrow, modifiers: .command)
            Button("Cooler") { night.nudge(level: -0.05) }.keyboardShortcut(.leftArrow, modifiers: .command)
            Button("Brighter") { night.nudge(brightness: 0.1) }.keyboardShortcut(.upArrow, modifiers: .command)
            Button("Dimmer") { night.nudge(brightness: -0.1) }.keyboardShortcut(.downArrow, modifiers: .command)
            Divider()
            Toggle("Follow the Sun", isOn: Binding(get: { night.followSun }, set: { night.setFollowSun($0) }))
        }
        CommandGroup(after: .appInfo) {
            Button("Check for Updates…") { Updates.shared.checkForUpdates() }
        }
        CommandGroup(replacing: .help) {
            Button("Good Night Help") { NSWorkspace.shared.open(URL(string: "https://github.com/michael-berardi/goodnight#readme")!) }
        }
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationWillFinishLaunching(_ note: Notification) {
        Night.shared.applyDockPolicy()
    }

    func applicationDidFinishLaunching(_ note: Notification) {
        Night.shared.start()
        HotKeys.register()
        Updates.shared.start()
        if Night.shared.showInDock {
            NSApp.activate(ignoringOtherApps: true)
        } else {
            // Menu bar only: start quietly, the window opens from the menu.
            DispatchQueue.main.async { NSApp.windows.first { $0.identifier?.rawValue == "main" }?.close() }
        }
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ app: NSApplication) -> Bool { false }

    func applicationShouldHandleReopen(_ app: NSApplication, hasVisibleWindows: Bool) -> Bool {
        if !hasVisibleWindows { Night.shared.openMain() }
        return true
    }

    func applicationWillTerminate(_ note: Notification) { Night.shared.stop() }
}

/// Signed updates through Sparkle: a daily check against the release feed (SUFeedURL), verified
/// with the EdDSA key in Info.plist and by code signing before anything is installed. Info.plist
/// sets the defaults: checks on, automatic install allowed but off.
@MainActor
final class Updates: ObservableObject {
    static let shared = Updates()
    @Published private(set) var reminder: String?
    // Sparkle holds its delegates weakly.
    private let driver = UpdateDriverDelegate()
    private let feed = UpdateFeedDelegate()
    private var controller: SPUStandardUpdaterController?
    private var observations: [NSKeyValueObservation] = []
    private var started = false

    var currentVersion: String {
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
    }

    func start() {
        let controller = SPUStandardUpdaterController(startingUpdater: false, updaterDelegate: feed, userDriverDelegate: driver)
        controller.updater.userAgentString = UpdatePolicy.userAgent
        controller.updater.httpHeaders = UpdatePolicy.headers
        // The prompt's checkbox changes these too; keep Settings in step.
        observations = [
            controller.updater.observe(\.automaticallyChecksForUpdates) { [weak self] _, _ in self?.changed() },
            controller.updater.observe(\.automaticallyDownloadsUpdates) { [weak self] _, _ in self?.changed() },
        ]
        self.controller = controller
        // Checks are background work, never startup work: the updater starts after a minute.
        DispatchQueue.main.asyncAfter(deadline: .now() + 60) { [weak self] in self?.ensureStarted() }
    }

    private nonisolated func changed() {
        DispatchQueue.main.async { MainActor.assumeIsolated { Updates.shared.objectWillChange.send() } }
    }

    private func ensureStarted() {
        guard !started, let updater = controller?.updater else { return }
        do {
            try updater.start()
            started = true
        } catch {
            NSLog("GoodNight: the updater failed to start: \(error.localizedDescription)")
        }
    }

    func checkForUpdates() {
        ensureStarted()
        NSApp.activate(ignoringOtherApps: true)
        controller?.checkForUpdates(nil)
    }

    var automatic: Bool {
        get { controller?.updater.automaticallyChecksForUpdates ?? true }
        set { controller?.updater.automaticallyChecksForUpdates = newValue }
    }

    var canInstallAutomatically: Bool { (controller?.updater.allowsAutomaticUpdates ?? false) && automatic }

    var installAutomatically: Bool {
        get { controller?.updater.automaticallyDownloadsUpdates ?? false }
        set { controller?.updater.automaticallyDownloadsUpdates = newValue }
    }

    /// A scheduled check in menu-bar-only use shows a quiet row instead of a dialog.
    func showReminder(_ version: String) {
        if ReleaseVersion.isNewer(version, than: currentVersion) { reminder = version }
    }

    func clearReminder() { reminder = nil }
}

/// Without a Dock icon the standard dialog would pop up over other work, so Sparkle hands
/// scheduled finds to this delegate, which shows a reminder row; Review opens the full prompt.
final class UpdateDriverDelegate: NSObject, SPUStandardUserDriverDelegate {
    var supportsGentleScheduledUpdateReminders: Bool { true }

    func standardUserDriverShouldHandleShowingScheduledUpdate(_ update: SUAppcastItem, andInImmediateFocus immediateFocus: Bool) -> Bool {
        MainActor.assumeIsolated { UpdatePolicy.standardDriverHandlesScheduledUpdate(showInDock: Night.shared.showInDock) }
    }

    func standardUserDriverWillHandleShowingUpdate(_ handleShowingUpdate: Bool, forUpdate update: SUAppcastItem, state: SPUUserUpdateState) {
        guard !handleShowingUpdate else { return }
        let version = update.displayVersionString
        DispatchQueue.main.async { MainActor.assumeIsolated { Updates.shared.showReminder(version) } }
    }

    func standardUserDriverDidReceiveUserAttention(forUpdate update: SUAppcastItem) {
        DispatchQueue.main.async { MainActor.assumeIsolated { Updates.shared.clearReminder() } }
    }

    func standardUserDriverWillFinishUpdateSession() {
        DispatchQueue.main.async { MainActor.assumeIsolated { Updates.shared.clearReminder() } }
    }
}

/// Only stable `x.y.z` releases are offered, and background checks stay off under CI or with
/// GOODNIGHT_NO_UPDATE_CHECK=1. GOODNIGHT_UPDATE_FEED points the updater at a test appcast (an
/// install from it still needs a valid EdDSA signature).
class UpdateFeedDelegate: NSObject, SPUUpdaterDelegate {
    func feedURLString(for updater: SPUUpdater) -> String? {
        ProcessInfo.processInfo.environment["GOODNIGHT_UPDATE_FEED"]
    }

    func updater(_ updater: SPUUpdater, mayPerform updateCheck: SPUUpdateCheck) throws {
        guard updateCheck == .updatesInBackground,
              !UpdatePolicy.backgroundChecksAllowed(environment: ProcessInfo.processInfo.environment) else { return }
        throw NSError(domain: "GoodNight", code: 1, userInfo: [NSLocalizedDescriptionKey: "Background update checks are turned off."])
    }

    func updater(_ updater: SPUUpdater, shouldProceedWithUpdate item: SUAppcastItem, updateCheck: SPUUpdateCheck) throws {
        guard ReleaseVersion.parts(item.displayVersionString) == nil else { return }
        // A prerelease or unparseable version counts as no update.
        throw NSError(domain: SUSparkleErrorDomain, code: Int(SUError.noUpdateError.rawValue),
                      userInfo: [NSLocalizedDescriptionKey: "Good Night is up to date."])
    }
}

/// `GoodNight --update-probe` checks the feed once without any UI and exits 0 when current,
/// 10 when an update is available and 1 when the check failed. It never downloads.
@MainActor
enum UpdateProbe {
    static func run() {
        let delegate = ProbeDelegate()
        let controller = SPUStandardUpdaterController(startingUpdater: false, updaterDelegate: delegate, userDriverDelegate: nil)
        controller.updater.userAgentString = UpdatePolicy.userAgent
        controller.updater.httpHeaders = UpdatePolicy.headers
        do {
            try controller.updater.start()
        } catch {
            print("Couldn't check for updates: \(error.localizedDescription)")
            exit(1)
        }
        controller.updater.checkForUpdateInformation()
        RunLoop.main.run(until: Date().addingTimeInterval(6))
        let current = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
        print("Install updates automatically: \(controller.updater.automaticallyDownloadsUpdates ? "on" : "off")")
        if let failure = delegate.failure {
            print("Couldn't check for updates: \(failure)")
            exit(1)
        }
        guard let version = delegate.found else {
            print("Good Night \(current) is up to date.")
            exit(0)
        }
        print(UpdateNotice.text(app: "Good Night", new: version, current: current))
        exit(10)
    }
}

private final class ProbeDelegate: UpdateFeedDelegate {
    private(set) var found: String?
    private(set) var failure: String?

    func updater(_ updater: SPUUpdater, didFindValidUpdate item: SUAppcastItem) {
        found = item.displayVersionString
    }

    func updater(_ updater: SPUUpdater, didFinishUpdateCycleFor updateCheck: SPUUpdateCheck, error: Error?) {
        // SUNoUpdateError (1001) only means nothing newer was found.
        guard let error = error as NSError?, !(error.domain == SUSparkleErrorDomain && error.code == 1001) else { return }
        failure = error.localizedDescription
    }
}

/// System-wide shortcuts: ⌃⌥⌘ with the arrow keys. They work even when the screen is dimmed to black.
enum HotKeys {
    private static var refs: [EventHotKeyRef?] = []

    static func register() {
        var spec = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
        InstallEventHandler(GetApplicationEventTarget(), { _, event, _ in
            var id = EventHotKeyID()
            GetEventParameter(event, EventParamName(kEventParamDirectObject), EventParamType(typeEventHotKeyID),
                              nil, MemoryLayout<EventHotKeyID>.size, nil, &id)
            DispatchQueue.main.async { MainActor.assumeIsolated { HotKeys.fire(id.id) } }
            return noErr
        }, 1, &spec, nil, nil)
        let mods = UInt32(controlKey | optionKey | cmdKey)
        for (i, key) in [kVK_UpArrow, kVK_DownArrow, kVK_RightArrow, kVK_LeftArrow].enumerated() {
            var ref: EventHotKeyRef?
            let status = RegisterEventHotKey(UInt32(key), mods, EventHotKeyID(signature: 0x474E_4954, id: UInt32(i)),
                                             GetApplicationEventTarget(), 0, &ref)
            if status != noErr { NSLog("GoodNight: shortcut \(i) is taken by another app (OSStatus \(status))") }
            refs.append(ref)
        }
    }

    @MainActor private static func fire(_ id: UInt32) {
        let n = Night.shared
        switch id {
        case 0: n.nudge(brightness: 0.1)
        case 1: n.nudge(brightness: -0.1)
        case 2: n.nudge(level: 0.05)
        default: n.nudge(level: -0.05)
        }
    }
}

/// `GoodNight --probe` prints the tint curve and schedule, so both can be checked without the UI.
@MainActor
enum Probe {
    static func run() {
        let n = Night.shared, f = DateFormatter()
        f.timeStyle = .short
        print(String(format: "%@ · sunrise %@ · sunset %@ · %@", n.placeName, f.string(from: n.sunrise), f.string(from: n.sunset), n.sunCaption))
        print("preset    level  kelvin   gain r  gain g  gain b   white  mid-grey  shadow")
        let ramp = (0..<256).map { Double($0) / 255 }
        for p in Preset.all {
            guard case let .warmth(kelvin, _, _, gains, _)? = try? n.engine.query(.warmth(level: p.level)),
                  case let .ramp(r, _, _)? = try? n.engine.query(.ramp(level: p.level, brightness: 1, r: ramp, g: ramp, b: ramp)) else { continue }
            print(String(format: "%@ %5.2f  %5.0fK   %.3f   %.3f   %.3f    %.3f   %.3f     %.3f",
                         p.name.padding(toLength: 9, withPad: " ", startingAt: 0), p.level, kelvin,
                         gains[0], gains[1], gains[2], r[255] / gains[0], r[128] / gains[0], r[26] / gains[0]))
        }
    }
}
