// swift-tools-version:5.9
import PackageDescription

// Legato's shared core for iPhone: the Rust library every phone runs
// (phone/android, built for iOS as a static library) and the Swift bindings
// UniFFI writes for it. Both are built by ios/scripts/build-core.sh and are
// not checked in.
let package = Package(
    name: "LegatoCore",
    platforms: [.iOS(.v17)],
    products: [
        .library(name: "LegatoCore", targets: ["LegatoCore"]),
    ],
    targets: [
        .binaryTarget(name: "LegatoCoreFFI", path: "LegatoCoreFFI.xcframework"),
        .target(
            name: "LegatoCore",
            dependencies: ["LegatoCoreFFI"],
            path: "Sources/LegatoCore",
            linkerSettings: [
                // What the Rust library asks the system for: the list
                // rustc prints as native-static-libs when it builds it.
                .linkedLibrary("iconv"),
                .linkedFramework("CoreFoundation"),
            ]
        ),
    ]
)
