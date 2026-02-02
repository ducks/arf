{ pkgs ? import <nixpkgs> {} }:

let
  rust-overlay = import (pkgs.fetchFromGitHub {
    owner = "oxalica";
    repo = "rust-overlay";
    rev = "5018343419ea808f8a413241381976b7e60951f2";
    sha256 = "19x56dqzplps4skxmqr2wdv03iyw9921gvjrlw1nqxfzh9w96334";
  });
  pkgsWithRust = import <nixpkgs> {
    overlays = [ rust-overlay ];
  };
in
pkgsWithRust.mkShell {
  buildInputs = with pkgsWithRust; [
    rust-bin.stable.latest.default
    rust-analyzer
  ];

  shellHook = ''
    echo ""
    echo "ARF Development Environment"
    echo "==========================="
    echo "Rust: $(rustc --version)"
    echo ""
  '';
}
