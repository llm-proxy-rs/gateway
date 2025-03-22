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
in
  pkgs.dockerTools.buildImage {
    name = "gateway";
    tag = "latest";
    config = {
      Cmd = ["server"];
    };
    copyToRoot = pkgs.buildEnv {
      name = "image-root";
      paths = [
        pkgs.dockerTools.caCertificates
        rustPackage
      ];
    };
  }
