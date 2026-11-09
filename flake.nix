{
  description = "Tandem development shell: pinned toolchains and tools";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages =
            (with pkgs; [
              rustc
              cargo
              clippy
              rustfmt
              rust-analyzer
              cargo-audit
              cargo-deny
              cargo-machete
              actionlint
              sqlx-cli
              just
              nodejs_22
              postgresql_18 # psql and pg_isready for local checks
              git
            ])
            ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [ pkgs.playwright-driver.browsers ];

          env = pkgs.lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux {
            PLAYWRIGHT_BROWSERS_PATH = "${pkgs.playwright-driver.browsers}";
            PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS = "true";
          };

          shellHook = ''
            expected="$(sed -n 's/^channel = "\(.*\)"/\1/p' rust-toolchain.toml 2>/dev/null)"
            actual="$(rustc --version | cut -d' ' -f2)"
            if [ -n "$expected" ] && [ "$expected" != "$actual" ]; then
              echo "warning: rust-toolchain.toml pins $expected but this shell provides $actual; update flake.lock or the pin" >&2
            fi
          '';
        };
      });
    };
}
