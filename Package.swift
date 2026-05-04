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
            url: "https://github.com/matt-harding/allourthings-core/releases/download/v1.0.1/allourthings_core.xcframework.zip",
            checksum: "f41d82f067e08ce7064e76bac8d894b6be795ddd0dd919785c7a59df34447b25"
        ),
        .target(
            name: "AllourthingsCore",
            dependencies: [.target(name: "allourthings_core")],
            path: "Sources/AllourthingsCore"
        ),
    ]
)
