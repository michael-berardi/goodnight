// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "GoodNight",
    platforms: [.macOS(.v13)],
    dependencies: [
        .package(url: "https://github.com/sparkle-project/Sparkle", from: "2.10.0"),
        .package(url: "https://github.com/michael-berardi/carapace", from: "0.1.0"),
    ],
    targets: [
        // The engine, built from ../core by scripts/build-core.sh.
        .binaryTarget(name: "GoodnightCore", path: "Core/GoodnightCore.xcframework"),
        .executableTarget(
            name: "GoodNight",
            dependencies: [
                .product(name: "Sparkle", package: "Sparkle"),
                .product(name: "CarapaceKit", package: "carapace"),
                .product(name: "CarapaceFFI", package: "carapace"),
                "GoodnightCore",
            ],
            path: "Sources/GoodNight",
            linkerSettings: [.unsafeFlags(["-Xlinker", "-rpath", "-Xlinker", "@executable_path/../Frameworks"])]
        )
    ]
)
