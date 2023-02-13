{
  inputs = {
    naersk.url = "github:nmattia/naersk/master";
    nixpkgs.url = "nixpkgs/nixos-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils, naersk, ... }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        naersk-lib = pkgs.callPackage naersk { };
      in
      {
        packages.default = naersk-lib.buildPackage {
          src = ./.;
          doCheck = true;
          pname = "litten";
          buildInputs = with pkgs; [ ];
        };

        # defaultApp.default = utils.lib.mkApp {
        #   drv = self.defaultPackage."${system}";
        # };

        devShells.default = with pkgs; mkShell {
          buildInputs = [ cargo rustc rustfmt ];
          RUST_SRC_PATH = rustPlatform.rustLibSrc;
        };
      });
}
