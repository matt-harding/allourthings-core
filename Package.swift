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
            url: "https://github.com/matt-harding/allourthings-core/releases/download/v0.1.11/allourthings_core.xcframework.zip",
            checksum: "272efc98c2d1e992c93441360ca74a05e21eb8c908323bb87eae887c29102cb6"
        ),
        .target(
            name: "AllourthingsCore",
            dependencies: [.target(name: "allourthings_core")],
            path: "Sources/AllourthingsCore"
        ),
    ]
)
