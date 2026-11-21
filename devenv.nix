{ pkgs, ... }:
{
  # keep-sorted start block=yes newline_separated=yes
  enterShell = ''
    prek_dir="$DEVENV_ROOT/tools/prek"
    if [ ! -x "$prek_dir/node_modules/.bin/prek" ] || [ "$prek_dir/package-lock.json" -nt "$prek_dir/node_modules/.package-lock.json" ]; then
      npm ci --ignore-scripts --no-audit --no-fund --prefix "$prek_dir" --silent
    fi
    export PATH="$prek_dir/node_modules/.bin:$PATH"
    prek install --quiet'';

  env = {
    # keep-sorted start
    # The AppHost's launcher finds the runtime through it, not PATH.
    DOTNET_ROOT = "${pkgs.dotnetCorePackages.sdk_10_0-bin}/share/dotnet";
    # keep-sorted end
  };

  languages = {
    # keep-sorted start block=yes newline_separated=yes
    rust = {
      # keep-sorted start block=yes prefix_order=enable
      enable = true;
      # https://github.com/cachix/devenv/blob/d59d872d80876d9eeb3e214d3b088bc4a14a9c4f/src/modules/languages/rust.nix#L311-L316
      channel = "stable";
      targets = [
        # keep-sorted start
        "wasm32-unknown-unknown"
        # keep-sorted end
      ];
      # keep-sorted end
    };
    # keep-sorted end
  };

  packages = with pkgs; [
    # keep-sorted start
    binaryen
    deadnix
    dioxus-cli
    dotnetCorePackages.sdk_10_0-bin
    # actionlint's and keep-sorted's hooks build them from source.
    go
    nixfmt
    nodejs_26
    sqlx-cli
    statix
    supabase-cli
    # keep-sorted end
  ];
  # keep-sorted end
}
