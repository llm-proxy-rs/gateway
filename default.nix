{pkgs ? import <nixpkgs> {}}: let
  rustPackage = pkgs.rustPlatform.buildRustPackage {
    buildInputs = [pkgs.openssl];
    cargoHash = "sha256-fBAqY4JKBclO5glN0NU/mJOe9EGCKT6nfMbrX97JmOA=";
    nativeBuildInputs = [pkgs.pkg-config];
    pname = "gateway";
    src = ./.;
    useFetchCargoVendor = true;
    version = "0.1.0";
  };
  entrypointScript = pkgs.writeScriptBin "entrypoint.sh" ''
    #!/bin/sh
    set -e

    echo "Running database migrations..."
    sqlx migrate run

    echo "Starting gateway server..."
    server
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
        pkgs.bash
        pkgs.dockerTools.caCertificates
        rustPackage
        sqlx-cli
      ];
    };
  }
