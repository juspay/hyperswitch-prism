{
  description = "Multi-language SDK development environment for the Connector Service Project";

  # Inputs are external dependencies/sources your flake depends on
  inputs = {
    # nixpkgs contains all the packages (like a package repository)
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    
    # flake-utils provides helper functions for multi-system support
    flake-utils.url = "github:numtide/flake-utils";
    
    # rust-overlay provides up-to-date Rust toolchains
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  # Outputs define what your flake provides
  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        # Create a package set with rust-overlay applied
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };

        # Define the Rust toolchain you want to use
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "clippy" "rustfmt" "rust-analyzer" ];
        };

      in
      {
        # Development shell - what you get when you run `nix develop`
        devShells.default = pkgs.mkShell {
          # Build inputs are available in the shell environment
          buildInputs = with pkgs; [
            rustToolchain
            
            # Additional development tools
            cargo-watch    # Auto-rebuild on file changes
            cargo-edit     # Commands like cargo add, cargo rm
            cargo-audit    # Security vulnerability scanner
            
            # System dependencies that some Rust crates might need
            pkg-config
            openssl

            # protobuf stuff
            protobuf        # protoc compiler
            protoc-gen-rust-grpc
            grpc-tools
            grpcurl
            # Node.js runtime and tools
            nodejs_20         # Node.js runtime for JavaScript SDK
            nodePackages.npm  # npm package manager

            # Python runtime and tools
            python3           # Python 3 runtime
            python3Packages.pip  # pip for installing Python deps
            python3Packages.grpcio  # gRPC support
            python3Packages.grpcio-tools  # protoc compiler for Python
            python3Packages.requests  # HTTP library for Python SDK
            python3Packages.mypy-protobuf  # Generate mypy stub files from protobuf specs
            python3Packages.jinja2  # Template engine for SDK codegen

            # Java/Gradle runtime and tools
            jdk17             # Java Development Kit (matches protobuf-java 4.x needs)
            gradle            # Gradle build tool

            # Optional: database tools if you're building web apps
            # postgresql
            # sqlite

            # Optional: if you need to link against system libraries
            # gcc
            # libiconv  # On macOS
          ];

          # Environment variables
          shellHook = "
            echo 'Rust development environment loaded!'
            echo \"Rust version: \$(rustc --version)\"
            echo \"Cargo version: \$(cargo --version)\"
            echo \"Node.js version: \$(node --version)\"
            echo \"Python version: \$(python3 --version)\"
            echo \"Java version: \$(java --version | head -1)\"
            echo \"Gradle version: \$(gradle --version | grep Gradle)\"

            # Optional: set environment variables
            export RUST_BACKTRACE=1
            export RUST_LOG=debug
          ";
        };
      }
    );
}
