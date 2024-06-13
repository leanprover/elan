nix build -o nixos.patch $(nix eval nixpkgs#elan.patches --apply "patches: (builtins.elemAt patches 0).outPath" --raw)
