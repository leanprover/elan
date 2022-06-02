<#
.SYNOPSIS
    .
.DESCRIPTION
    This is just a little script that can be downloaded from the internet to
    install elan. It just does platform detection, downloads the installer
    and runs it.
.PARAMETER Verbose
    Produce verbose output about the elan installation process.
.PARAMETER NoPrompt
    Do not present elan installation menu of choices.
.PARAMETER NoModifyPath
    Do not modify PATH environment variable.
.PARAMETER DefaultToolchain
    Which tool chain to setup as your default toolchain, or specify 'none'
.PARAMETER ElanRoot
    Whee to find the elan-init tool, default is https://github.com/leanprover/elan/release.
#>
param(
    [bool]$Verbose = 0,
    [bool]$NoPrompt = 0,
    [bool]$NoModifyPath = 0,
    [string]$DefaultToolchain = "",
    [string]$ElanRoot = "https://github.com/leanprover/elan/releases"
)


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
    $x = New-Item -ItemType Directory -Path $_dir
}
$_file = "$_dir/elan-init$_ext"

Write-Host "info: downloading installer to ${temp}"

$x = Get-RedirectedUrl "https://github.com/leanprover/elan/releases/latest"
$xs =  -split $x -split '/'
$_latest = $xs[-1]
$x = Invoke-WebRequest -Uri "$ElanRoot/download/$_latest/elan-$_arch.zip" -OutFile "$_dir/elan-init.zip"
$x = Expand-Archive -Path "$_dir/elan-init.zip" -DestinationPath "$_dir" -Force

$cmdline = " "
if ($DefaultToolchain -ne "") {
    $cmdline += "--default-toolchain $DefaultToolchain"
}
if ($NoPrompt){
    $cmdline += " -y"
}
if ($NoModifyPath){
    $cmdline += " --no-modify-path"
}
if ($Verbose){
    $cmdline += " --verbose"
}
$details = Start-Process -FilePath "$_dir/elan-init.exe" -ArgumentList $cmdline -Wait -NoNewWindow -Passthru

$rc = $details.exitCode
if ($rc -ne 0 ) {
    Write-Host "Elan failed with error code $rc"
    return 1
}

$rx = Remove-Item -Recurse -Force "$_dir"

return 0
