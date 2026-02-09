{
  description = "ThinCell dev shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
    verus-flake = {
      url = "github:stephen-huan/verus-flake";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-overlay.follows = "rust-overlay";
    };
  };

  outputs =
    {
      nixpkgs,
      rust-overlay,
      flake-utils,
      verus-flake,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        verusPkgs = verus-flake.packages.${system};
        verusfmtForZed = pkgs.writeShellScriptBin "verusfmtForZed" ''
          tmp=$(mktemp --suffix=.rs)
          cat > "$tmp"
          ${verusPkgs.verusfmt}/bin/verusfmt "$tmp"
          cat "$tmp"
        '';
      in
      with pkgs;
      {
        devShells.default = mkShell {
          buildInputs = [
            bacon
            verusfmtForZed
            verusPkgs.verusfmt
            verusPkgs.rustup
            verusPkgs.verus
            verusPkgs.vargo
            (rust-bin.selectLatestNightlyWith (
              toolchain:
              toolchain.default.override {
                extensions = [
                  "rust-src"
                  "miri"
                ];
              }
            ))
          ];
        };
      }
    );
}
