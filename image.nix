# Production container image for the `web` fullstack server.
#
# The server binary is compiled by `dx build --release` inside the devenv shell,
# so it is dynamically linked against nixpkgs glibc with a /nix/store ELF
# interpreter that does not exist in an ordinary container. This expression wraps
# that prebuilt artifact with autoPatchelfHook — which rewrites the interpreter
# and rpath to in-store libraries and records them as runtime dependencies — and
# then bundles the resulting closure into an OCI image via dockerTools. The exact
# glibc the binary needs therefore travels inside the image (correct by
# construction; no glibc-version coupling, no static musl toolchain).
#
# Build in CI with `nix-build image.nix` (non-flake on purpose, so the gitignored
# target/ artifact is visible as a relative-path src). The output is a gzipped OCI
# archive that `docker load` or `skopeo` can push to a registry. This is meant to
# run on x86_64-linux (the CI runner and Render); building it on macOS needs a
# linux builder.
{
  # Pinned to the same nixpkgs revision devenv resolves (the `nixpkgs` node in
  # devenv.lock) so the bundled glibc matches the compiler that built the binary.
  # The hash is that node's narHash, which for a GitHub source matches what
  # fetchTarball computes. If nix ever reports a mismatch, paste the hash it
  # prints as the expected value.
  nixpkgs ? builtins.fetchTarball {
    url = "https://github.com/cachix/devenv-nixpkgs/archive/6004ea8c229fe9d41b21c6f4c76bf6c2e10771dd.tar.gz";
    sha256 = "sha256-LZhOm4kqjHUnwP4CNHpaqqEVcumGNd1PvOEOad8LkS0=";
  },
  pkgs ? import nixpkgs { system = "x86_64-linux"; },
  # Directory produced by `dx bundle --package web --platform web --release`
  # (contains the `server` binary next to `public/`).
  artifact ? ./target/dx/web/release/web,
  tag ? "latest",
}:
let
  # Wrap the prebuilt dx output: patch the ELF interpreter/rpath to in-store libs
  # and stage `server` + `public/` under /app.
  server = pkgs.stdenv.mkDerivation {
    pname = "maximal-lottery-web";
    version = "0.1.0";
    src = artifact;
    dontUnpack = true;
    nativeBuildInputs = [ pkgs.autoPatchelfHook ];
    # glibc comes from stdenv; cc.cc.lib provides libgcc_s / libstdc++. Add more
    # here only if autoPatchelf reports a missing library.
    buildInputs = [ pkgs.stdenv.cc.cc.lib ];
    installPhase = ''
      runHook preInstall
      mkdir -p "$out/app"
      cp -r "$src"/. "$out/app/"
      chmod -R u+w "$out/app"
      # The dx fullstack web output is `public/` plus a single server executable.
      # Normalize its name to `server` regardless of the crate's binary name so
      # the image Cmd is stable.
      exe="$(find "$out/app" -maxdepth 1 -type f | head -n1)"
      if [ -z "$exe" ]; then
        echo "no server binary found in $out/app" >&2
        exit 1
      fi
      [ "$exe" = "$out/app/server" ] || mv "$exe" "$out/app/server"
      chmod +x "$out/app/server"
      runHook postInstall
    '';
  };
in
pkgs.dockerTools.buildLayeredImage {
  name = "maximal-lottery-web";
  inherit tag;
  # cacert provides a CA bundle; the binary is also built with webpki-roots, so
  # TLS to Supabase works either way.
  contents = [
    server
    pkgs.cacert
  ];
  config = {
    Cmd = [ "/app/server" ];
    WorkingDir = "/app";
    Env = [
      # Bind all interfaces; Render injects PORT at runtime.
      "IP=0.0.0.0"
      "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
    ];
    ExposedPorts = {
      "8080/tcp" = { };
    };
  };
}
