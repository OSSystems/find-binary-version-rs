{
  description = "find-binary-version";

  inputs = {
    nixpkgs.url = "nixpkgs/nixos-22.05";

    rust = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-utils = {
      url = "github:numtide/flake-utils";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, rust, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        rust-toolchain = with rust.packages.${system};
          let
            channel_1_59_0 = {
              channel = "1.59.0";
              sha256 = "sha256-4IUZZWXHBBxcwRuQm9ekOwzc0oNqH/9NkI1ejW7KajU=";
            };
            stable_1_59_0 = toolchainOf channel_1_59_0;
          in
          combine [
            (stable_1_59_0.withComponents [ "rustc" "cargo" "rust-src" "clippy" ])

            latest.rustfmt
            latest.rust-analyzer
          ];
      in
      {
        devShell = pkgs.mkShell {
          buildInputs = with pkgs; [
            rust-toolchain
            cargo-release

            pkg-config
            libarchive
          ];
        };
      });
}
