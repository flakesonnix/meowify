{
  description = "Rust SoundCloud desktop/TUI client development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      devShells = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        {
          default = pkgs.mkShell {
            nativeBuildInputs = with pkgs; [
              cargo
              cargo-nextest
              clang
              clippy
              git
              just
              pkg-config
              rustc
              rustfmt
            ];

            buildInputs = with pkgs; [
              avahi
              bluez
              dbus
              glib
              gst_all_1.gst-libav
              gst_all_1.gst-plugins-bad
              gst_all_1.gst-plugins-base
              gst_all_1.gst-plugins-good
              gst_all_1.gst-plugins-ugly
              gst_all_1.gstreamer
              gtk4
              libadwaita
              openssl
              sqlite
            ];

            RUST_BACKTRACE = "1";
            MEOWIFY_DEV_SHELL = "1";

            shellHook = ''
              echo "meowify dev shell: cargo, GTK4, libadwaita, GStreamer, SQLite, BlueZ ready"
            '';
          };
        });
    };
}
