# How to build

Important: For all `leanpkg build` invocations, set `--target` (even if the target is the same as the host architecture), because that affects the output directory. Pass the same target also via `-Target` to `build.ps1` in step 3.

## Steps

1) Build the main project with the `--features "msi-installed"` flag, resulting in `elan-init.exe`
2) Build the CustomAction DLL in `src/elan-win-installer` using `leanpkg build`
3) Build the actual installer in `src/elan-win-installer/msi` using `build.ps1`

The resulting installer will be in `src/elan-win-installer/msi/target`.
