// swift-tools-version: 5.9

// Vendored copy of KeyboardKit 8.9.6 (MIT) with a patch for iOS 27 host
// bundle id lookup. See README.md in this folder.

import PackageDescription

let package = Package(
    name: "KeyboardKit",
    defaultLocalization: "en",
    platforms: [
        .iOS(.v14),
        .macOS(.v11),
        .tvOS(.v14),
        .watchOS(.v7),
        .visionOS(.v1)
    ],
    products: [
        .library(
            name: "KeyboardKit",
            targets: ["KeyboardKit"]
        )
    ],
    targets: [
        .target(
            name: "KeyboardKit",
            dependencies: [],
            resources: [.process("Resources")]
        )
    ]
)
