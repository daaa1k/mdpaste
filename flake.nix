{
  description = "mdpaste — Paste clipboard image as Markdown link";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
  };

  outputs = { self, nixpkgs, flake-utils, crane }:
    let
      # Read version from Cargo.toml to keep it in sync automatically.
      version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;

      # Pre-built binary hashes for each supported platform.
      # Update these whenever a new version is released:
      #   nix store prefetch-file --hash-type sha256 --json <url>
      binaryHashes = {
        "x86_64-linux"   = "sha256-5atdZk4+gLJkhgpmRjRRh2NXjcJ23k3RL4aKZximlX4=";
        "aarch64-darwin" = "sha256-wKuQXPE5Wtt5+hGkqurj0gtFWAGoSw1xdULVt2skqSc=";
      };

      # Map Nix system strings to GitHub Release artifact names.
      binaryArtifacts = {
        "x86_64-linux"   = "mdpaste-linux-x86_64";
        "aarch64-darwin" = "mdpaste-macos-aarch64";
      };

      # Build a package wrapping the pre-built GitHub Release binary.
      #
      # On Linux (including WSL2 + NixOS), autoPatchelfHook rewrites the ELF
      # interpreter and RPATH so the binary works under the Nix store layout.
      mkBinaryPackage = pkgs:
        let
          system   = pkgs.stdenv.hostPlatform.system;
          artifact = binaryArtifacts.${system}
            or (throw "mdpaste-bin: no pre-built binary for ${system}");
          hash     = binaryHashes.${system};
          src = pkgs.fetchurl {
            url = "https://github.com/daaa1k/mdpaste/releases/download/v${version}/${artifact}";
            inherit hash;
          };
        in
        pkgs.stdenv.mkDerivation {
          pname = "mdpaste-bin";
          inherit version src;

          dontUnpack = true;

          # autoPatchelfHook is only needed on Linux (including WSL2/NixOS).
          nativeBuildInputs = pkgs.lib.optionals pkgs.stdenv.isLinux [
            pkgs.autoPatchelfHook
          ];

          # Runtime libraries required by the Linux binary (glibc + libgcc_s).
          buildInputs = pkgs.lib.optionals pkgs.stdenv.isLinux [
            pkgs.glibc
            pkgs.gcc.cc.lib
          ];

          installPhase = ''
            install -Dm755 $src $out/bin/mdpaste
          '';
        };

      # Home Manager module — system-agnostic, exported at the top level.
      #
      # Usage in a Home Manager configuration:
      #
      #   inputs.mdpaste.url = "github:daaa1k/mdpaste";
      #
      #   { inputs, ... }: {
      #     imports = [ inputs.mdpaste.homeManagerModules.default ];
      #     programs.mdpaste = {
      #       enable = true;
      #       # Use the pre-built binary instead of building from source:
      #       # package = inputs.mdpaste.packages.${pkgs.system}.mdpaste-bin;
      #       settings = {
      #         backend = "r2";
      #         r2 = {
      #           account_id = "your-account-id";
      #           # endpoint = "https://your-account-id.r2.cloudflarestorage.com"; # optional
      #         };
      #         # R2 credentials are read from R2_ACCESS_KEY_ID / R2_SECRET_ACCESS_KEY env vars.
      #       };
      #     };
      #   }
      hmModule = { config, lib, pkgs, ... }:
        let
          cfg = config.programs.mdpaste;
          tomlFormat = pkgs.formats.toml { };
        in
        {
          options.programs.mdpaste = {
            enable = lib.mkEnableOption "mdpaste clipboard image to Markdown link tool";

            package = lib.mkOption {
              type = lib.types.package;
              default = self.packages.${pkgs.system}.default;
              defaultText = lib.literalExpression "mdpaste.packages.\${pkgs.system}.default";
              description = ''
                The mdpaste package to install.

                Two variants are available:
                - `mdpaste.packages.''${pkgs.system}.default` — built from source via Crane (default)
                - `mdpaste.packages.''${pkgs.system}.mdpaste-bin` — pre-built binary from GitHub Releases
                  (faster setup; no Rust compilation required; supports x86_64-linux and aarch64-darwin)
              '';
            };

            settings = lib.mkOption {
              type = tomlFormat.type;
              default = { };
              description = ''
                Global configuration for mdpaste written to
                {file}`$XDG_CONFIG_HOME/mdpaste/config.toml`.

                Top-level keys:
                - `backend` — default backend (`"r2"` or `"local"`)
                - `r2` — Cloudflare R2 credentials (`account_id`, `access_key`, `secret_key`, `endpoint`)
                - `wsl` — WSL2 executable paths (`powershell_path`, `win32yank_path`)

                See the mdpaste README for the full schema reference.
              '';
              example = lib.literalExpression ''
                {
                  backend = "r2";
                  r2 = {
                    account_id = "your-account-id";
                    # endpoint = "https://your-account-id.r2.cloudflarestorage.com"; # optional
                    # R2 credentials via R2_ACCESS_KEY_ID / R2_SECRET_ACCESS_KEY env vars.
                  };
                  # wsl = {
                  #   powershell_path = "/mnt/c/Program Files/PowerShell/7/pwsh.exe";
                  # };
                }
              '';
            };
          };

          config = lib.mkIf cfg.enable {
            home.packages = [ cfg.package ];

            xdg.configFile."mdpaste/config.toml" = lib.mkIf (cfg.settings != { }) {
              source = tomlFormat.generate "mdpaste-config.toml" cfg.settings;
            };
          };
        };
    in
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        craneLib = crane.mkLib pkgs;

        # Arguments shared between dependency pre-build and the final build.
        commonArgs = {
          src = craneLib.cleanCargoSource ./.;
          strictDeps = true;

          # libiconv is required on macOS.
          buildInputs = pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
          ];
        };

        # Pre-build only the dependencies to maximise cache reuse.
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        mdpaste = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
        });
      in
      {
        # --- packages ---------------------------------------------------
        packages = {
          default = mdpaste;
          inherit mdpaste;
        } // pkgs.lib.optionalAttrs (binaryArtifacts ? ${system}) {
          # mdpaste-bin is only exposed on platforms that have a pre-built binary.
          mdpaste-bin = mkBinaryPackage pkgs;
        };

        # --- checks (run by `nix flake check`) --------------------------
        checks = {
          # Build the package itself.
          inherit mdpaste;

          # Check formatting with rustfmt.
          mdpaste-fmt = craneLib.cargoFmt {
            src = craneLib.cleanCargoSource ./.;
          };

          # Run clippy with --deny warnings.
          mdpaste-clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          });

          # Run the test suite.
          mdpaste-test = craneLib.cargoTest (commonArgs // {
            inherit cargoArtifacts;
          });
        };

        # --- devShell ---------------------------------------------------
        devShells.default = craneLib.devShell {
          checks = self.checks.${system};
          packages = [
            pkgs.rust-analyzer
            pkgs.rustfmt
          ];
        };
      }
    ) // {
      # --- Home Manager module (system-agnostic) ----------------------
      homeManagerModules.default = hmModule;
    };
}
