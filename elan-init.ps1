
# This is just a little script that can be downloaded from the internet to
# install elan. It just does platform detection, downloads the installer
# and runs it.


$ELAN_UPDATE_ROOT="https://github.com/leanprover/elan/releases"

#XXX: If you change anything here, please make the same changes in setup_mode.rs
function usage() {
    Write-Host "
elan-init 1.0.0 (408ed84 2017-02-11)
The installer for elan

USAGE:
    elan-init [FLAGS] [OPTIONS]

FLAGS:
    -v, --verbose           Enable verbose output
    -y                      Disable confirmation prompt.
        --no-modify-path    Don't configure the PATH environment variable
    -h, --help              Prints help information
    -V, --version           Prints version information

OPTIONS:
        --default-toolchain <default-toolchain>    Choose a default toolchain to install
        --default-toolchain none                   Do not install any toolchains
"
}

Function Get-RedirectedUrl {
    Param (
        [Parameter(Mandatory=$true)]
        [String]$url
    )

    $request = [System.Net.WebRequest]::Create($url)
    $request.AllowAutoRedirect=$true
    $request.UserAgent = 'Mozilla/5.0 (Windows NT; Windows NT 10.0; en-US) AppleWebKit/534.6 (KHTML, like Gecko) Chrome/7.0.500.0 Safari/534.6'

    try
    {
        $response = $request.GetResponse()
        $response.ResponseUri.AbsoluteUri
        $response.Close()
    }
    catch
    {
        "Error: $_"
    }
}

function main($cmdline) {

    $cputype=[System.Environment]::GetEnvironmentVariable("PROCESSOR_ARCHITECTURE");

    if ($cputype -ne "AMD64") {
        Write-Host "### Elan install only supports 64 bit windows with AMD64 architecture"
        return 1
    }

    $_arch="x86_64-pc-windows-msvc"
    $_ext = ".exe"
    $temp = [System.IO.Path]::GetTempPath()
    $_dir = Join-Path $temp "elan"
    if (-not (Test-Path -Path $_dir)) {
        New-Item -ItemType Directory -Path $_dir
    }
    $_file = "$_dir/elan-init$_ext"

    Write-Host "info: downloading installer to ${temp}"

    $x = Get-RedirectedUrl "https://github.com/leanprover/elan/releases/latest"
    $xs =  -split $x -split '/'
    $_latest = $xs[-1]
    Invoke-WebRequest -Uri "$ELAN_UPDATE_ROOT/download/$_latest/elan-$_arch.zip" -OutFile "$_dir/elan-init.zip"
    Expand-Archive -Path "$_dir/elan-init.zip" -DestinationPath "$_dir" -Force

    if ($cmdline.Count -eq 0) {
        Start-Process -FilePath "$_dir/elan-init.exe" -Wait -NoNewWindow
    } else {
        Start-Process -FilePath "$_dir/elan-init.exe" -ArgumentList $cmdline -Wait -NoNewWindow
    }

    if( -not $? ) {
        Write-Host "Elan failed with error code $?"
        return 1
    }

    Remove-Item -Recurse -Force "$_dir"

    return 0
}

$rc = main -cmdline $args
Exit $rc
