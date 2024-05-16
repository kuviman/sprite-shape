{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    geng.url = "github:geng-engine/cargo-geng";
    geng.inputs.nixpkgs.follows = "nixpkgs";
  };
  outputs = { geng, nixpkgs, ... }@inputs: geng.makeFlakeOutputs (system:
    let
      pkgs = import nixpkgs { inherit system; };
      pkgs-stable = import inputs.nixpkgs-stable { inherit system; };
    in
    {
      src = ./.;
      extraBuildInputs = [ pkgs.caddy pkgs.kdialog ];
    });
}
