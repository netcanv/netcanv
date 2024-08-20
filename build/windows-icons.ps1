# Modify these if the executable location ever changes.
$app_executable = "target/release/netcanv.exe"
$relay_executable = "target/release/netcanv-relay.exe"

# Helper for retrieving the version of the crate. This is used to set the product version.
function Get-CrateVersion {
   $metadata = cargo.exe metadata --no-deps --format-version 1 | ConvertFrom-Json
   return $metadata.packages[0].version
}

Write-Output "Obtaining crate versions"
$app_version = Get-CrateVersion
Set-Location netcanv-relay
$relay_version = Get-CrateVersion
Set-Location ..
Write-Output "netcanv: $app_version"
Write-Output "netcanv-relay: $relay_version"

# Generate the copyright info according to the current year.
$company_name = "NetCanv"
$year = Get-Date -Format "yyyy"
$copyright = "Copyright (c) $year liquidev and contributors"

# Download rcedit.
If (-not (Test-Path -Path "rcedit.exe" -PathType Leaf)) {
   Write-Output "Downloading rcedit"
   $rcedit_url = "https://github.com/electron/rcedit/releases/download/v1.1.1/rcedit-x64.exe"
   (New-Object System.Net.WebClient).DownloadFile($rcedit_url, "rcedit.exe")
}

function Set-Resources {
   param (
      $Executable,
      $Name,
      $Version,
      $Description
   )
   $original_filename = Split-Path $Executable -Leaf
   ./rcedit.exe "$Executable" `
      --set-icon resources/netcanv.ico `
      --set-version-string ProductName "$Name" `
      --set-version-string FileDescription "$Description" `
      --set-product-version "$Version" `
      --set-file-version "$Version" `
      --set-version-string CompanyName "$company_name" `
      --set-version-string LegalCopyright "$copyright" `
      --set-version-string OriginalFilename "$original_filename"
}

Write-Output "Applying resources to executables"

Set-Resources -Executable $app_executable `
   -Name "NetCanv" `
   -Version "$app_version" `
   -Description "Online collaborative paint canvas"

Set-Resources -Executable $relay_executable `
   -Name "NetCanv Relay" `
   -Version "$relay_version" `
   -Description "Relay server for NetCanv"
