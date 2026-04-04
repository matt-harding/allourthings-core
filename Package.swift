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
            url: "https://github.com/matt-harding/allourthings-core/releases/download/v0.1.9/allourthings_core.xcframework.zip",
            checksum: "ca3c7deb2e518fbea61ed7c330849e5f5945c40fe7807c6c3403bbf96f176ad1"
        ),
        .target(
            name: "AllourthingsCore",
            dependencies: [.target(name: "allourthings_core")],
            path: "Sources/AllourthingsCore"
        ),
    ]
)
