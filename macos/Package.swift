// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "GoodNight",
    platforms: [.macOS(.v13)],
    dependencies: [
        .package(url: "https://github.com/sparkle-project/Sparkle", from: "2.10.0")
    ],
    targets: [
        .executableTarget(
            name: "GoodNight",
            dependencies: [.product(name: "Sparkle", package: "Sparkle")],
            path: "Sources/GoodNight",
            linkerSettings: [.unsafeFlags(["-Xlinker", "-rpath", "-Xlinker", "@executable_path/../Frameworks"])]
        )
    ]
)
