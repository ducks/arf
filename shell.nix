{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    rustc
    cargo
    rust-analyzer
    clippy
    rustfmt
  ];

  shellHook = ''
    echo ""
    echo "ARF Development Environment"
    echo "==========================="
    echo "Rust: $(rustc --version)"
    echo ""
    echo "Commands:"
    echo "  cargo build    - Build arf"
    echo "  cargo run      - Run arf"
    echo "  cargo test     - Run tests"
    echo ""
  '';
}
