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
  server = pkgs.stdenv.mkDerivation {
    buildInputs = pkgs.lib.optionals pkgs.stdenv.isLinux (
      with pkgs;
      [
        stdenv.cc.cc.lib
      ]
    );
    dontUnpack = true;
    installPhase = ''
      runHook preInstall
      mkdir -p "$out/app"
      cp -r "$src"/. "$out/app/"
      chmod +x "$out/app/server"
      runHook postInstall
    '';
    # Linux-only inputs so the Nix expression can evaluate on macOS.
    nativeBuildInputs = pkgs.lib.optionals pkgs.stdenv.isLinux (
      with pkgs;
      [
        autoPatchelfHook
      ]
    );
    pname = "maximal-lottery-web";
    src = artifact;
    version = "0.1.0";
  };
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
