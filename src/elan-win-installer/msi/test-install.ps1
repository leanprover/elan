# This script can be used for manually testing the MSI installer. It is not used for AppVeyor CI.

$env:RUSTFLAGS="-Zunstable-options -Ctarget-feature=+crt-static"

pushd ..\..\..
# Build elan.exe
cargo build --release --target i686-pc-windows-msvc --features msi-installed
popd
if($LastExitCode -ne 0) { exit $LastExitCode }
pushd ..
# Build the CA library
cargo build --release --target i686-pc-windows-msvc
popd
if($LastExitCode -ne 0) { exit $LastExitCode }
# Build the MSI
.\build.ps1 -Target i686-pc-windows-msvc
if($LastExitCode -ne 0) { exit $LastExitCode }
# Run the MSI with logging
$OLD_LEANPKG_HOME = $env:LEANPKG_HOME
$OLD_ELAN_HOME = $env:ELAN_HOME
$env:LEANPKG_HOME = "$env:USERPROFILE\.leanpkg-test"
$env:ELAN_HOME = "$env:USERPROFILE\.elan-test"
Start-Process msiexec -ArgumentList "/i target\elan.msi /L*V target\Install.log" -Wait
$env:LEANPKG_HOME = $OLD_LEANPKG_HOME
$env:ELAN_HOME = $OLD_ELAN_HOME