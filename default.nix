{pkgs ? import <nixpkgs> {}}: let
  rustPackage = pkgs.rustPlatform.buildRustPackage {
    buildInputs = [pkgs.openssl];
    cargoHash = "sha256-2v37iedRFdzgChI2ZeedxkjdSQ9nN395shAGYWaLE6o=";
    nativeBuildInputs = [pkgs.pkg-config];
    pname = "gateway";
    src = ./.;
    useFetchCargoVendor = true;
    version = "0.1.0";
  };
  entrypointScript = pkgs.writeScriptBin "entrypoint.sh" ''
    #!${pkgs.bash}/bin/bash
    set -e

    echo "Running database migrations..."
    ${pkgs.sqlx-cli}/bin/sqlx migrate run

    echo "Starting gateway server..."
    exec ${rustPackage}/bin/server
  '';
  migrations = pkgs.runCommand "migrations" {} ''
    mkdir -p $out/migrations
    cp -r ${./migrations}/* $out/migrations/
  '';
in
  pkgs.dockerTools.buildImage {
    name = "gateway";
    tag = "latest";
    config = {
      Entrypoint = ["${entrypointScript}/bin/entrypoint.sh"];
    };
    copyToRoot = pkgs.buildEnv {
      name = "image-root";
      paths = [
        entrypointScript
        migrations
        pkgs.dockerTools.caCertificates
      ];
    };
  }
