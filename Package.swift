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
            // CI replaces this path with a remote url + checksum on each release.
            path: "build/allourthings_core.xcframework"
        ),
        .target(
            name: "AllourthingsCore",
            dependencies: [.target(name: "allourthings_core")],
            path: "Sources/AllourthingsCore"
        ),
    ]
)
