# Uninstall currently installed version of elan. Does the same thing as `elan self uninstall`.

$key = 'HKCU:\SOFTWARE\elan'
$productCode = (Get-ItemProperty -Path $key -Name InstalledProductCode).InstalledProductCode

# No need to set LEANPKG_HOME, because the installation directory is stored in the registry
$OLD_ELAN_HOME = $env:ELAN_HOME
$env:ELAN_HOME = "$env:USERPROFILE\.elan-test"
msiexec /x "$productCode" /L*V "target\Uninstall.log"
$env:ELAN_HOME = $OLD_ELAN_HOME