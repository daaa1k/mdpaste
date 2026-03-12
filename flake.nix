{
  description = "mdpaste — Paste clipboard image as Markdown link";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
  };

  outputs = { self, nixpkgs, flake-utils, crane }:
    let
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
      #       settings = {
      #         backend = "r2";
      #         r2 = {
      #           account_id = "your-account-id";
      #           access_key = "your-access-key";
      #           secret_key = "your-secret-key";
      #         };
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
              description = "The mdpaste package to install.";
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
                    access_key = "your-access-key";
                    secret_key = "your-secret-key";
                    # endpoint = "https://your-account-id.r2.cloudflarestorage.com";
                  };
                  # wsl = {
                  #   powershell_path = "/mnt/c/Program Files/PowerShell/7/pwsh.exe";
                  #   win32yank_path  = "/mnt/c/Users/you/AppData/Local/Microsoft/WinGet/Links/win32yank.exe";
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
