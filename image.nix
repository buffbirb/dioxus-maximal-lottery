{
  artifact ? ./target/dx/web/release/web,
  nixpkgs ?
    let
      locked = (builtins.fromJSON (builtins.readFile ./devenv.lock)).nodes.nixpkgs.locked;
    in
    builtins.fetchTarball {
      sha256 = locked.narHash;
      url = "https://github.com/${locked.owner}/${locked.repo}/archive/${locked.rev}.tar.gz";
    },
  pkgs ? import nixpkgs { system = builtins.currentSystem; },
  tag ? "latest",
}:
let
  server = pkgs.stdenv.mkDerivation (
    {
      dontUnpack = true;
      installPhase = ''
        runHook preInstall
        mkdir -p "$out/app"
        cp -r "$src"/. "$out/app/"
        chmod +x "$out/app/server"
        runHook postInstall
      '';
      pname = "maximal-lottery-web";
      src = artifact;
      version = "0.1.0";
    }
    // pkgs.lib.optionalAttrs pkgs.stdenv.isLinux {
      buildInputs = with pkgs; [
        stdenv.cc.cc.lib
      ];
      nativeBuildInputs = with pkgs; [
        autoPatchelfHook
      ];
    }
  );
in
pkgs.dockerTools.buildLayeredImage {
  config = {
    Cmd = [
      "/app/server"
    ];
    WorkingDir = "/app";
  };
  contents = [
    server
  ];
  name = "maximal-lottery-web";
  inherit tag;
}
