// swift-tools-version: 5.9
import PackageDescription

// DEVELOPMENT: run scripts/build-ios.sh first to create build/allourthings_core.xcframework.
// RELEASE: CI patches the binaryTarget below to a remote URL + checksum before tagging.
let package = Package(
    name: "AllourthingsCore",
    platforms: [.iOS(.v17)],
    products: [
        .library(name: "AllourthingsCore", targets: ["AllourthingsCore"]),
    ],
    targets: [
        .binaryTarget(
            name: "allourthings_core",
            url: "https://github.com/matt-harding/allourthings-core/releases/download/v2.1.0/allourthings_core.xcframework.zip",
            checksum: "2d0e3375c5a6795109f945d4894f22db9a0e32f255089273209cfc524f6e753f"
        ),
        .target(
            name: "AllourthingsCore",
            dependencies: [.target(name: "allourthings_core")],
            path: "Sources/AllourthingsCore"
        ),
    ]
)
