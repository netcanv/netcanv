#!/usr/bin/env bash

wget -nc https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
chmod +x linuxdeploy-x86_64.AppImage

# Notes for future generations:
# --appdir: specifies linuxdeploy's output directory, where to create the AppDir.
# --executable: specifies the executable file.
# --icon-file and --desktop-file are self-explanatory.
# --output appimage: specifies that an AppImage should be generated.

./linuxdeploy-x86_64.AppImage \
  --appdir NetCanv-AppDir \
  --executable target/release/netcanv \
  --icon-file resources/icon/16/netcanv.png \
  --icon-file resources/icon/32/netcanv.png \
  --icon-file resources/icon/64/netcanv.png \
  --icon-file resources/icon/128/netcanv.png \
  --icon-file resources/icon/256/netcanv.png \
  --desktop-file resources/netcanv.desktop \
  --output appimage

mkdir bin
mv NetCanv-*.AppImage bin/NetCanv-linux-$(uname -m).AppImage
rm -r NetCanv*-AppDir
