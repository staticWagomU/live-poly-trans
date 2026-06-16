{
  description = "LivePolyTrans development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    { nixpkgs, ... }:
    let
      systems = [
        "aarch64-darwin"
        "x86_64-darwin"
      ];

      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      devShells = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell {
            SDKROOT = "/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk";

            packages = [
              pkgs.bun
              pkgs.cargo
              pkgs.cargo-flamegraph
              pkgs.rustc
              pkgs.rustfmt
              pkgs.clippy
              pkgs.rust-analyzer
              pkgs.pkg-config
            ];

            shellHook = ''
              echo "LivePolyTrans dev shell: bun $(bun --version), cargo $(cargo --version | awk '{print $2}'), flamegraph $(cargo flamegraph --version | awk '{print $2}')"
            '';
          };
        }
      );
    };
}
