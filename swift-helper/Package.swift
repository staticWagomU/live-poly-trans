// swift-tools-version: 6.2
import PackageDescription

let package = Package(
  name: "LivePolyTransHelper",
  platforms: [.macOS(.v26)],
  products: [
    .executable(name: "live-poly-trans-helper", targets: ["LivePolyTransHelper"])
  ],
  targets: [
    .target(
      name: "AudioRingBuffer",
      publicHeadersPath: "include",
      cxxSettings: [
        .unsafeFlags(["-std=c++20"])
      ]
    ),
    .executableTarget(
      name: "LivePolyTransHelper",
      dependencies: ["AudioRingBuffer"],
      linkerSettings: [
        .unsafeFlags([
          "-Xlinker", "-sectcreate",
          "-Xlinker", "__TEXT",
          "-Xlinker", "__info_plist",
          "-Xlinker", "Info.plist"
        ])
      ]
    )
  ]
)
