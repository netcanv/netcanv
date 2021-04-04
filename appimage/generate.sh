#!/bin/bash

cargo build --release;

cd ../netcanv-matchmaker;
cargo build --release;

cd ..;

cd appimage;
wget -nc https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage;
chmod +x ./linuxdeploy-x86_64.AppImage;

mkdir out;
cd out;

rm -rf *;

../linuxdeploy-x86_64.AppImage --appdir AppDir_mm --executable ../../target/release/netcanv-matchmaker --desktop-file ../netcanv-matchmaker.desktop -i ../netcanv.png --output appimage;
../linuxdeploy-x86_64.AppImage --appdir AppDir_app --executable ../../target/release/netcanv --desktop-file ../netcanv.desktop -i ../netcanv.png --output appimage;

cd ..;
