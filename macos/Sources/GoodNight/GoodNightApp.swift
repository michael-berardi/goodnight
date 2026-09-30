import AppKit
import Carbon.HIToolbox
import Sparkle
import SwiftUI

@main
struct GoodNightApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    @StateObject private var night = Night.shared
    @AppStorage("showInMenuBar") private var showInMenuBar = true

    init() {
        if CommandLine.arguments.contains("--probe") { Probe.run(); exit(0) }
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
func keepWindowOpener(_ openWindow: OpenWindowAction) {
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

/// Signed updates through Sparkle: a daily check against the GitHub release feed (SUFeedURL),
/// verified with the EdDSA key in Info.plist before anything is installed.
final class Updates: ObservableObject {
    static let shared = Updates()
    private var controller: SPUStandardUpdaterController?

    func start() {
        controller = SPUStandardUpdaterController(startingUpdater: true, updaterDelegate: nil, userDriverDelegate: nil)
    }

    func checkForUpdates() {
        NSApp.activate(ignoringOtherApps: true)
        controller?.checkForUpdates(nil)
    }

    var automatic: Bool {
        get { controller?.updater.automaticallyChecksForUpdates ?? true }
        set { controller?.updater.automaticallyChecksForUpdates = newValue; objectWillChange.send() }
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
            DispatchQueue.main.async { HotKeys.fire(id.id) }
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

    private static func fire(_ id: UInt32) {
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
enum Probe {
    static func run() {
        let e = Sun.events(on: Date()), f = DateFormatter()
        f.timeStyle = .short
        print(String(format: "Place %.2f, %.2f (%@) · sunrise %@ · sunset %@ · nightness %.2f",
                     Sun.place.lat, Sun.place.lon, Sun.placeName, f.string(from: e.rise), f.string(from: e.set), Sun.nightness()))
        print("preset    level  kelvin   gain r  gain g  gain b   white  mid-grey  shadow")
        for p in Preset.all {
            let g = Warmth.gains(p.level), d = Float(Warmth.dim(p.level))
            print(String(format: "%@ %5.2f  %5.0fK   %.3f   %.3f   %.3f    %.3f   %.3f     %.3f",
                         p.name.padding(toLength: 9, withPad: " ", startingAt: 0), p.level, Warmth.kelvin(p.level),
                         g.r, g.g, g.b, Display.knee(1, d), Display.knee(0.5, d), Display.knee(0.1, d)))
        }
    }
}
