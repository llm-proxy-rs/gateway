{pkgs ? import <nixpkgs> {}}: let
  rustPackage = pkgs.rustPlatform.buildRustPackage {
    buildInputs = [pkgs.openssl];
    cargoHash = "sha256-HMbvrjSwj3NKHLzT30Px0e1oq5wYrcy/gKY0tzH9jP4=";
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
