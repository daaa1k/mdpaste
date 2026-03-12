{ pkgs, lib, ... }:
{
  cachix.push = "daaa1k";

  languages.rust = {
    enable = true;
    channel = "stable";
  };

  packages = with pkgs; [
    rust-analyzer
  ];
}
