{ pkgs ? import <nixpkgs> {} }:

with pkgs;

stdenv.mkDerivation {
  name = "find-binary-version";
  buildInputs = [
    pkg-config
    libarchive
  ];
}
