{
  description = "dioxus-maximal-lottery development shell";

  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];

      perSystem =
        { pkgs, ... }:
        let
          inherit (inputs.nixpkgs) lib;
          dotnet =
            assert lib.assertMsg (lib.versionAtLeast dotnetSdk.version sdkVersion)
              "nixpkgs has .NET SDK ${dotnetSdk.version}; global.json needs ${sdkVersion}";
            dotnetSdk;
          dotnetSdk = pkgs.dotnetCorePackages."sdk_${lib.replaceStrings [ "." ] [ "_" ] sdkChannel}-bin";
          node =
            assert lib.assertMsg (
              pkgs ? "nodejs_${nodeMajor}"
            ) "nixpkgs has no nodejs_${nodeMajor}; .node-version needs it";
            pkgs."nodejs_${nodeMajor}";
          nodeMajor = lib.versions.major (lib.trim (builtins.readFile ./.node-version));
          rust = (inputs.rust-overlay.lib.mkRustBin { } pkgs).fromRustupToolchainFile ./rust-toolchain.toml;
          sdkChannel = lib.versions.majorMinor sdkVersion;
          sdkVersion = (lib.importJSON ./global.json).sdk.version;
        in
        {
          devShells = {
            default = pkgs.mkShell {
              env = {
                DOTNET_CLI_TELEMETRY_OPTOUT = 1;
                DOTNET_NOLOGO = 1;
                # The AppHost's launcher finds the runtime through it, not PATH.
                DOTNET_ROOT = "${dotnet}/share/dotnet";
              };

              packages = with pkgs; [
                cargo-binstall
                deadnix
                dotnet
                # actionlint's hook builds it from source.
                go
                nixfmt
                node
                rust
                # actionlint checks workflow scripts with it, as on the runners.
                shellcheck
                statix
              ];

              shellHook = ''
                root="$(git rev-parse --show-toplevel)"
                "$root/tools/setup.sh" "$root/.tools"
                export PATH="$root/.tools/bin:$PATH"
              ''
              # dx's macOS release links Homebrew's OpenSSL by absolute path, and
              # the Aspire CLI's hardened runtime strips DYLD_* fallbacks.
              + lib.optionalString pkgs.stdenv.hostPlatform.isDarwin ''
                if [ ! -e /opt/homebrew/opt/openssl@3/lib/libssl.3.dylib ]; then
                  echo "dx needs Homebrew's OpenSSL: brew install openssl@3" >&2
                fi
              '';
            };
          };

          formatter = pkgs.nixfmt;
        };
    };
}
