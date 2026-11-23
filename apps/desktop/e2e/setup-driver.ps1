# Puts the Microsoft Edge WebDriver that matches the installed WebView2 runtime into
# apps/desktop/e2e/.driver/msedgedriver.exe and checks that tauri-driver is installed.
#
#   powershell -File apps/desktop/e2e/setup-driver.ps1           # driver for the WebView2 runtime
#   powershell -File apps/desktop/e2e/setup-driver.ps1 -UseEdge  # driver for the installed Edge
#
# The WebView2 version is the one that has to match. If no session starts with it (the e2e run
# fails at "create session"), run the script again with -UseEdge. The version in place is written
# to .driver/version.txt.
param([switch]$UseEdge)

$ErrorActionPreference = 'Stop'

# Download pattern of the official page
# https://developer.microsoft.com/en-us/microsoft-edge/tools/webdriver/ (checked 2026-11-23).
# {0} is the full version, for example 157.0.4341.60.
$DriverUrl = 'https://msedgedriver.microsoft.com/{0}/edgedriver_win64.zip'

# EdgeUpdate client ids: WebView2 runtime and Microsoft Edge (stable).
$WebView2Client = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
$EdgeClient = '{56EB18F8-B008-4CBD-B6D2-8C97FE7E9062}'

function Get-ClientVersion([string]$client) {
    foreach ($root in 'HKLM:\SOFTWARE\WOW6432Node', 'HKCU:\SOFTWARE') {
        $key = "$root\Microsoft\EdgeUpdate\Clients\$client"
        $pv = (Get-ItemProperty -Path $key -Name pv -ErrorAction SilentlyContinue).pv
        if ($pv -and $pv -ne '0.0.0.0') { return $pv }
    }
    return $null
}

if ($UseEdge) {
    $source = 'Microsoft Edge'
    $version = Get-ClientVersion $EdgeClient
} else {
    $source = 'WebView2 runtime'
    $version = Get-ClientVersion $WebView2Client
}
if (-not $version) {
    Write-Error "No $source version found in the registry."
}

$driverDir = Join-Path $PSScriptRoot '.driver'
$driver = Join-Path $driverDir 'msedgedriver.exe'
New-Item -ItemType Directory -Force -Path $driverDir | Out-Null

# A driver is used only with a valid Authenticode signature by Microsoft, checked before it is
# ever run. Returns the signer, or $null.
function Get-MicrosoftSigner([string]$path) {
    $sig = Get-AuthenticodeSignature -FilePath $path
    $subject = if ($sig.SignerCertificate) { $sig.SignerCertificate.Subject } else { '' }
    if ($sig.Status -eq 'Valid' -and $subject -match '(^|, )O=Microsoft Corporation(,|$)') {
        return $subject
    }
    Write-Host "Signature check failed for ${path}: status $($sig.Status), signer '$subject'"
    return $null
}

# The four-part version msedgedriver reports, e.g. 157.0.4341.60.
function Get-DriverVersion([string]$path) {
    return ((& $path --version) -split ' ' | Where-Object { $_ -match '^\d+\.\d+\.\d+\.\d+$' } | Select-Object -First 1)
}

$signer = $null
if ((Test-Path $driver) -and ($signer = Get-MicrosoftSigner $driver) -and ((Get-DriverVersion $driver) -eq $version)) {
    Write-Host "msedgedriver $version is already in place ($source)."
} else {
    $zip = Join-Path $driverDir 'edgedriver_win64.zip'
    $unpacked = Join-Path $driverDir 'unpacked'
    $url = $DriverUrl -f $version
    Write-Host "Downloading $url"
    Invoke-WebRequest -Uri $url -OutFile $zip -UseBasicParsing
    if (Test-Path $unpacked) { Remove-Item -Recurse -Force $unpacked }
    Expand-Archive -Path $zip -DestinationPath $unpacked
    Remove-Item -Force $zip
    $candidate = Join-Path $unpacked 'msedgedriver.exe'
    $signer = Get-MicrosoftSigner $candidate
    if (-not $signer) {
        Remove-Item -Recurse -Force $unpacked
        Write-Error 'The downloaded msedgedriver.exe is not validly signed by Microsoft. Nothing was installed.'
    }
    $reported = Get-DriverVersion $candidate
    if ($reported -ne $version) {
        Remove-Item -Recurse -Force $unpacked
        Write-Error "The downloaded driver reports version '$reported', expected $version. Nothing was installed."
    }
    Copy-Item -Force $candidate $driver
    Remove-Item -Recurse -Force $unpacked
}
$stamp = (Get-Date).ToString('yyyy-MM-ddTHH:mm:ssK')
Set-Content -Path (Join-Path $driverDir 'version.txt') -Encoding ascii -Value @(
    "msedgedriver $version for the $source",
    "version reported by the driver: $(Get-DriverVersion $driver)",
    "Authenticode: Valid, signer $signer",
    "verified $stamp"
)
& $driver --version

# tauri-driver 2.1.0 has no --version flag: presence is checked, the version comes from cargo.
if (Get-Command tauri-driver -ErrorAction SilentlyContinue) {
    $installed = (cargo install --list) | Select-String -Pattern '^tauri-driver v'
    Write-Host "tauri-driver found: $installed"
} else {
    Write-Host 'tauri-driver is missing. Install it with: cargo install tauri-driver --locked'
    exit 1
}
