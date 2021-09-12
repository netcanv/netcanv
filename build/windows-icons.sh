#!/usr/bin/env bash

app_executable="target/release/netcanv.exe"
matchmaker_executable="target/release/netcanv-matchmaker.exe"

get-crate-version() {
  cargo metadata --no-deps --format-version 1 | jq -r .packages[0].version
}

app_version="$(get-crate-version)"
cd netcanv-matchmaker
matchmaker_version="$(get-crate-version)"
cd ..

year="$(date +%Y)"
company_name="liquidev"
copyright="Copyright (c) $year liquidev and contributors"

wget -nc "https://github.com/electron/rcedit/releases/download/v1.1.1/rcedit-x64.exe"

apply-resources() {
  filename=$1
  product_name=$2
  product_description=$3
  ./rcedit-x64.exe "$filename" \
    --set-icon resources/netcanv.ico \
    --set-version-string ProductName "$product_name" \
    --set-version-string FileDescription "$product_description" \
    --set-product-version "$app_version" \
    --set-file-version "$app_version" \
    --set-version-string CompanyName "$company_name" \
    --set-version-string LegalCopyright "$copyright" \
    --set-version-string OriginalFilename "$(basename $app_executable)"
}

apply-resources target/release/netcanv.exe \
  "NetCanv" "Online collaborative paint canvas"

apply-resources target/release/netcanv-matchmaker.exe \
  "NetCanv Matchmaker" "Matchmaker and packet relay server for NetCanv"

