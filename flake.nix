{
  description = "crawk - Dependency crawler for Rust crates";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs = { self, nixpkgs, rust-overlay, crane }:
    let
      system = "x86_64-linux";

      pkgs = import nixpkgs {
        inherit system;
        overlays = [ rust-overlay.overlays.default ];
      };

      # Toolchain from rust-toolchain.toml (Rust 1.95.0)
      rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

      # Dev toolchain with extras
      rustToolchainDev = rustToolchain.override {
        extensions = [ "rust-src" "rust-analyzer" "clippy" "rustfmt" ];
      };

      # Crane lib configured with our toolchain
      craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

      # Common source filtering — only include Rust-relevant files
      src = craneLib.cleanCargoSource ./.;

      # Native dependencies needed by vergen-git2 (libgit2 bindings)
      nativeBuildInputs = with pkgs; [ pkg-config cmake ];
      buildInputs = with pkgs; [ openssl libgit2 zlib ];

      commonArgs = {
        inherit src nativeBuildInputs buildInputs;
        inherit (craneLib.crateNameFromCargoToml { inherit src; }) pname version;

        # Vergen fallbacks (no .git / clamped SOURCE_DATE_EPOCH in nix sandbox)
        CRAWK_GIT_DESCRIBE = if self ? rev then self.shortRev else "dirty";
        CRAWK_BUILD_TIMESTAMP =
          let d = self.lastModifiedDate; in
          "${builtins.substring 0 4 d}-${builtins.substring 4 2 d}-${builtins.substring 6 2 d}T${builtins.substring 8 2 d}:${builtins.substring 10 2 d}:${builtins.substring 12 2 d}Z";
      };

      # Phase 1: build only dependencies (cached separately)
      cargoArtifacts = craneLib.buildDepsOnly commonArgs;

      # Phase 2: build the actual crate (reuses cached deps)
      crawk = craneLib.buildPackage (commonArgs // {
        inherit cargoArtifacts;
        doCheck = false;
      });
    in
    {
      packages.${system} = {
        default = crawk;
        inherit crawk;
      };

      devShells.${system}.default = pkgs.mkShell {
        inherit nativeBuildInputs;

        buildInputs = buildInputs ++ (with pkgs; [
          rustToolchainDev
          cargo-edit
          cargo-watch
          cargo-insta
          cargo-nextest
          just
        ]);

        RUST_SRC_PATH = "${rustToolchainDev}/lib/rustlib/src/rust/library";
        LIBGIT2_NO_VENDOR = "1";
      };
    };
}
