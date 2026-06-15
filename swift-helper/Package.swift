// swift-tools-version: 6.2
import PackageDescription

let package = Package(
  name: "LivePolyTransHelper",
  platforms: [.macOS(.v26)],
  products: [
    .executable(name: "live-poly-trans-helper", targets: ["LivePolyTransHelper"])
  ],
  targets: [
    .executableTarget(name: "LivePolyTransHelper")
  ]
)
