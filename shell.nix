{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
    name = "rust-env";
    nativeBuildInputs = with pkgs; [
        rustc cargo
    ];
    buildInputs = with pkgs; [ xorg.libxcb python3 ];

    RUST_BACKTRACE = 1;
}
