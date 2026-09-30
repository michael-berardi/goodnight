import AppKit
import SwiftUI

private let fade = Animation.easeInOut(duration: 1)

// MARK: Sky — the hero. Animatable, so SwiftUI interpolates `level` frame by frame.

struct Sky: View, Animatable {
    var level: Double
    /// Where the sun and moon rest (fractions of the frame) and their size relative to height.
    var sunX = 0.76, moonX = 0.22, restY = 0.37, bodySize = 0.16

    var animatableData: Double { get { level } set { level = newValue } }

    private static let keys: [(l: Double, top: (Double, Double, Double), bottom: (Double, Double, Double))] = [
        (0.00, (0.29, 0.55, 0.91), (0.70, 0.84, 0.98)),
        (0.45, (0.33, 0.29, 0.56), (0.98, 0.60, 0.40)),
        (0.75, (0.09, 0.09, 0.24), (0.52, 0.24, 0.26)),
        (1.00, (0.02, 0.03, 0.08), (0.10, 0.07, 0.16)),
    ]

    private func sky() -> (Color, Color) {
        let i = max(1, Sky.keys.firstIndex { $0.l >= level } ?? Sky.keys.count - 1)
        let a = Sky.keys[i - 1], b = Sky.keys[i]
        let t = clamp((level - a.l) / (b.l - a.l))
        func mix(_ x: (Double, Double, Double), _ y: (Double, Double, Double)) -> Color {
            Color(red: x.0 + (y.0 - x.0) * t, green: x.1 + (y.1 - x.1) * t, blue: x.2 + (y.2 - x.2) * t)
        }
        return (mix(a.top, b.top), mix(a.bottom, b.bottom))
    }

    var body: some View {
        let (top, bottom) = sky()
        let setting = smoothstep(0, 0.6, level), rising = smoothstep(0.45, 0.85, level)
        GeometryReader { g in
            let w = g.size.width, h = g.size.height, d = h * bodySize
            ZStack {
                LinearGradient(colors: [top, bottom], startPoint: .top, endPoint: .bottom)
                Canvas { ctx, size in
                    var seed: UInt64 = 7
                    for _ in 0..<70 {
                        seed = seed &* 6364136223846793005 &+ 1442695040888963407
                        let x = Double(seed >> 40 & 0xFFFF) / 65535 * size.width
                        let y = Double(seed >> 20 & 0xFFFF) / 65535 * size.height * 0.7
                        let r = 0.5 + Double(seed >> 8 & 0xFF) / 255 * 1.1
                        ctx.fill(Path(ellipseIn: CGRect(x: x, y: y, width: r, height: r)), with: .color(.white.opacity(0.85)))
                    }
                }
                .opacity(smoothstep(0.55, 0.95, level))
                Circle()
                    .fill(Color(red: 1, green: 0.97 - 0.35 * setting, blue: 0.88 - 0.6 * setting))
                    .frame(width: d, height: d)
                    .shadow(color: Color(red: 1, green: 0.75, blue: 0.4).opacity(0.35 + 0.4 * setting), radius: d * 0.6)
                    .position(x: w * sunX, y: h * restY + h * 0.75 * setting)
                    .opacity(1 - smoothstep(0.5, 0.7, level))
                Image(systemName: "moon.fill")
                    .font(.system(size: d * 0.8))
                    .foregroundStyle(Color(red: 1, green: 0.95, blue: 0.86))
                    .shadow(color: .white.opacity(0.35), radius: d * 0.35)
                    .position(x: w * moonX, y: h * restY + h * 0.8 * (1 - rising))
                    .opacity(rising)
            }
        }
        .clipped()
    }
}

// MARK: Dial — the one slider style, used for warmth and brightness.

struct Dial: View {
    var value: Double
    var stops: [Gradient.Stop]
    var knob: Color
    var detents: [Double] = []
    var label: String
    var set: (Double, Bool) -> Void

    var body: some View {
        GeometryReader { g in
            let size: CGFloat = 24, span = g.size.width - size
            ZStack(alignment: .leading) {
                Capsule()
                    .fill(LinearGradient(stops: stops, startPoint: .leading, endPoint: .trailing))
                    .overlay(Capsule().strokeBorder(Color.primary.opacity(0.12), lineWidth: 0.5))
                    .frame(height: 8)
                    .padding(.horizontal, size / 2 - 4)
                Circle()
                    .fill(.white)
                    .overlay(Circle().fill(knob).padding(6))
                    .shadow(color: .black.opacity(0.25), radius: 2, y: 1)
                    .frame(width: size, height: size)
                    .offset(x: span * value)
            }
            .frame(maxHeight: .infinity)
            .contentShape(Rectangle())
            .gesture(DragGesture(minimumDistance: 0).onChanged { v in
                var x = clamp((v.location.x - size / 2) / span)
                // Soft magnetic detents.
                if let p = detents.first(where: { abs($0 - x) < 0.012 }) {
                    if abs(p - value) > 0.0001 { NSHapticFeedbackManager.defaultPerformer.perform(.alignment, performanceTime: .now) }
                    x = p
                }
                set(x, false)
            })
        }
        .frame(height: 28)
        .accessibilityRepresentation {
            Slider(value: Binding(get: { value }, set: { set($0, true) })) { Text(label) }
        }
    }
}

struct WarmthDial: View {
    @EnvironmentObject var night: Night
    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "sun.max.fill").foregroundStyle(.secondary).font(.system(size: 13)).frame(width: 16)
            Dial(value: night.level, stops: Warmth.trackStops, knob: Warmth.color(night.level),
                 detents: Preset.all.map(\.level), label: "Warmth") { night.setLevel($0, animated: $1) }
            Image(systemName: "moon.fill").foregroundStyle(.secondary).font(.system(size: 12)).frame(width: 16)
        }
        .help("Warmth · \(night.kelvin.formatted())K")
    }
}

struct BrightnessDial: View {
    @EnvironmentObject var night: Night
    var body: some View {
        // Bright end stays a readable warm white even at Midnight.
        let white = Warmth.color(min(night.effectiveLevel, 0.3))
        HStack(spacing: 10) {
            Image(systemName: "sun.min.fill").foregroundStyle(.secondary).font(.system(size: 11)).frame(width: 16)
            Dial(value: night.brightness, stops: [.init(color: .black, location: 0), .init(color: white, location: 1)],
                 knob: white.opacity(0.25 + 0.75 * night.brightness), detents: [1], label: "Brightness") { night.setBrightness($0, animated: $1) }
            Image(systemName: "sun.max.fill").foregroundStyle(.secondary).font(.system(size: 13)).frame(width: 16)
        }
        .help("Brightness · \(Int((night.brightness * 100).rounded()))%")
    }
}

// MARK: Presets

struct PresetRow: View {
    @EnvironmentObject var night: Night
    var size: CGFloat = 38
    var body: some View {
        HStack(spacing: 0) {
            ForEach(Preset.all) { p in
                let on = night.enabled && abs(night.level - p.level) < 0.01
                Button { withAnimation(fade) { night.setLevel(p.level, animated: true) } } label: {
                    VStack(spacing: 5) {
                        Image(systemName: on ? p.symbol + ".fill" : p.symbol)
                            .font(.system(size: size * 0.4, weight: .medium))
                            .foregroundStyle(on ? (p.level > 0.6 ? Color.white : Color.black.opacity(0.8)) : Color.primary)
                            .frame(width: size, height: size)
                            .background(Circle().fill(on ? AnyShapeStyle(Warmth.color(p.level)) : AnyShapeStyle(.quaternary)))
                            .overlay(Circle().strokeBorder(Color.primary.opacity(on ? 0.15 : 0), lineWidth: 0.5))
                        Text(p.name).font(.caption2).foregroundStyle(on ? .primary : .secondary)
                    }
                    .frame(maxWidth: .infinity)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .help("\(p.name) · \(Int((Warmth.kelvin(p.level) / 50).rounded() * 50))K")
            }
        }
    }
}

/// Label on the left, switch on the right, full width.
struct Row<Label: View>: View {
    @Binding var isOn: Bool
    @ViewBuilder var label: Label
    var body: some View {
        HStack {
            label
            Spacer()
            Toggle("", isOn: $isOn).toggleStyle(Switch()).labelsHidden()
        }
    }
}

/// A switch that reads the same in any window state. Menu bar windows never become key,
/// so the system switch would look disabled there.
struct Switch: ToggleStyle {
    func makeBody(configuration c: Configuration) -> some View {
        Capsule()
            .fill(c.isOn ? AnyShapeStyle(LinearGradient(colors: [Color(red: 1, green: 0.72, blue: 0.42), Color(red: 0.96, green: 0.5, blue: 0.26)],
                                                           startPoint: .leading, endPoint: .trailing))
                         : AnyShapeStyle(Color.primary.opacity(0.18)))
            .overlay(Capsule().strokeBorder(Color.primary.opacity(0.08), lineWidth: 0.5))
            .frame(width: 40, height: 24)
            .overlay(alignment: c.isOn ? .trailing : .leading) {
                Circle().fill(.white).shadow(color: .black.opacity(0.25), radius: 1.5, y: 1).padding(2)
            }
            .contentShape(Capsule())
            .onTapGesture { withAnimation(.easeOut(duration: 0.18)) { c.isOn.toggle() } }
            .accessibilityRepresentation { Toggle(isOn: c.$isOn) { c.label } }
    }
}

struct FollowSunRow: View {
    @EnvironmentObject var night: Night
    var body: some View {
        Row(isOn: Binding(get: { night.followSun }, set: { v in withAnimation(fade) { night.setFollowSun(v) } })) {
            VStack(alignment: .leading, spacing: 2) {
                Text("Follow the sun")
                Text(night.sunCaption).font(.caption).foregroundStyle(.secondary)
            }
        }
    }
}

func powerBinding(_ night: Night) -> Binding<Bool> {
    Binding(get: { night.enabled }, set: { v in withAnimation(fade) { night.setEnabled(v) } })
}

/// Opens Settings on every supported macOS version.
struct SettingsButton: View {
    var body: some View {
        if #available(macOS 14.0, *) {
            SettingsLink { Text("Settings…") }.simultaneousGesture(TapGesture().onEnded { NSApp.activate(ignoringOtherApps: true) })
        } else {
            Button("Settings…") {
                NSApp.activate(ignoringOtherApps: true)
                NSApp.sendAction(Selector(("showPreferencesWindow:")), to: nil, from: nil)
            }
        }
    }
}

// MARK: Menu bar popover

struct MenuView: View {
    @EnvironmentObject var night: Night
    var body: some View {
        VStack(spacing: 14) {
            ZStack(alignment: .bottomLeading) {
                Sky(level: night.effectiveLevel, sunX: 0.8, moonX: 0.7, restY: 0.3, bodySize: 0.22)
                HStack(alignment: .bottom) {
                    VStack(alignment: .leading, spacing: 0) {
                        Text(night.title)
                            .font(.system(size: 30, weight: .light, design: .rounded))
                            .contentTransition(.numericText())
                        Text(night.subtitle).font(.subheadline.weight(.medium)).opacity(0.85)
                    }
                    Spacer()
                    Toggle("Good Night", isOn: powerBinding(night)).toggleStyle(Switch()).labelsHidden()
                }
                .foregroundStyle(.white)
                .shadow(color: .black.opacity(0.25), radius: 6)
                .padding(14)
            }
            .frame(height: 118)
            .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))

            WarmthDial()
            BrightnessDial()
            PresetRow()
            Divider()
            FollowSunRow()
            Divider()
            HStack(spacing: 16) {
                Button("Open Good Night") { night.openMain() }
                Spacer()
                SettingsButton()
                Button("Quit") { NSApp.terminate(nil) }
            }
            .buttonStyle(.plain)
            .foregroundStyle(.secondary)
        }
        .padding(14)
        .frame(width: 300)
    }
}

// MARK: Main window

struct MainView: View {
    @EnvironmentObject var night: Night
    @Environment(\.openWindow) private var openWindow
    var body: some View {
        ZStack(alignment: .bottom) {
            Sky(level: night.effectiveLevel).ignoresSafeArea()
            VStack(spacing: 0) {
                VStack(spacing: 2) {
                    Text(night.title)
                        .font(.system(size: 72, weight: .thin, design: .rounded))
                        .contentTransition(.numericText())
                    Text(night.subtitle).font(.title3.weight(.medium)).opacity(0.85)
                }
                .foregroundStyle(.white)
                .shadow(color: .black.opacity(0.25), radius: 10)
                .padding(.top, 56)
                Spacer()
                VStack(spacing: 18) {
                    HStack {
                        Text("Good Night").font(.headline)
                        Spacer()
                        Toggle("Good Night", isOn: powerBinding(night)).toggleStyle(Switch()).labelsHidden()
                    }
                    WarmthDial()
                    BrightnessDial()
                    PresetRow(size: 44)
                    Divider()
                    FollowSunRow()
                }
                .padding(20)
                .modifier(Glass())
                .padding(14)
            }
        }
        .frame(width: 400, height: 660)
        .onAppear { keepWindowOpener(openWindow) }
    }
}

private struct Glass: ViewModifier {
    func body(content: Content) -> some View {
        if #available(macOS 26.0, *) {
            content.glassEffect(.regular, in: RoundedRectangle(cornerRadius: 26, style: .continuous))
        } else {
            content.background(.regularMaterial, in: RoundedRectangle(cornerRadius: 26, style: .continuous))
        }
    }
}

// MARK: Settings

struct SettingsView: View {
    @EnvironmentObject var night: Night
    @ObservedObject private var updates = Updates.shared
    @AppStorage("showInMenuBar") private var showInMenuBar = true
    @AppStorage("showInDock") private var showInDock = true
    var body: some View {
        let e = Sun.events(on: Date()), f = DateFormatter()
        let _ = f.timeStyle = .short
        Form {
            Section {
                Toggle("Show in menu bar", isOn: $showInMenuBar).disabled(!showInDock)
                Toggle("Show in Dock", isOn: $showInDock).disabled(!showInMenuBar)
                    .onChange(of: showInDock) { _ in night.applyDockPolicy() }
                Toggle("Open at login", isOn: Binding(get: { night.launchAtLogin }, set: { night.setLaunchAtLogin($0) }))
                if let error = night.loginError { Text(error).foregroundStyle(.red).font(.callout) }
            }
            Section("Schedule") {
                Toggle("Follow the sun", isOn: Binding(get: { night.followSun }, set: { v in withAnimation(fade) { night.setFollowSun(v) } }))
                LabeledContent("Location", value: Sun.placeName)
                LabeledContent("Today", value: "Sunrise \(f.string(from: e.rise)) · Sunset \(f.string(from: e.set))")
            }
            Section("Updates") {
                Toggle("Check for updates automatically", isOn: Binding(get: { updates.automatic }, set: { updates.automatic = $0 }))
                LabeledContent("Version", value: Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "")
            }
            Section("Keyboard") {
                LabeledContent("Warmer · Cooler", value: "⌃⌥⌘→ · ⌃⌥⌘←")
                LabeledContent("Brighter · Dimmer", value: "⌃⌥⌘↑ · ⌃⌥⌘↓")
            }
        }
        .formStyle(.grouped)
        .scrollDisabled(true)
        .frame(width: 440)
        .fixedSize()
    }
}
