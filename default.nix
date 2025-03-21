{pkgs ? import <nixpkgs> {}}: let
  rustPackage = pkgs.rustPlatform.buildRustPackage {
    buildInputs = [pkgs.openssl];
    cargoHash = "sha256-4Uyz94tL7HFjuve15nG9eaWnbOVPSeVygN8t8tGpb6U=";
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
