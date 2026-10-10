<#
.SYNOPSIS
    Earth Studio'yu tek satırla kurar (Windows, yönetici izni gerekmez).
    Installs Earth Studio with one line (Windows, no administrator rights).

.DESCRIPTION
    En son EarthStudio-Kurulum.exe'yi GitHub Releases'tan indirir, SHA-256
    özetini doğrular ve sessizce çalıştırır: Earth Studio kurulur, masaüstüne
    ve Başlat menüsüne kısayol eklenir, .earth dosyaları Studio'da açılır.
    PowerShell ile indirilen dosyalar SmartScreen uyarısı göstermez.

    Kurulum / Install:
        irm https://raw.githubusercontent.com/MahmutSibal/earth-lang/main/install.ps1 | iex

    Kaldırma / Uninstall: Ayarlar > Uygulamalar > Earth Studio, ya da
        & ([scriptblock]::Create((irm https://raw.githubusercontent.com/MahmutSibal/earth-lang/main/install.ps1))) -Uninstall
#>
param(
    # Kurulacak sürüm etiketi, örneğin v0.5.0. Boşsa en son sürüm.
    [string]$Version = "",
    # Sürüm dosyalarının indirileceği adres (test için).
    [string]$BaseUrl = "",
    [switch]$Uninstall
)

$ErrorActionPreference = "Stop"
$Repository = "MahmutSibal/earth-lang"
$Asset = "EarthStudio-Kurulum.exe"
# Must match AppId in windows/installer.iss.
$UninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{6F0E7C52-8D1B-4E6A-9C3F-EA27B5D4E1A9}_is1"
$Turkish = (Get-Culture).TwoLetterISOLanguageName -eq "tr"

function Say([string]$Tr, [string]$En) {
    if ($Turkish) { Write-Host $Tr } else { Write-Host $En }
}

function Fail([string]$Tr, [string]$En) {
    if ($Turkish) { throw $Tr } else { throw $En }
}

# Versions up to 0.4 were installed by this script itself, with a command-line
# earth.exe on PATH. Remove that layout so only Earth Studio remains.
function Remove-OldInstall {
    $old = Join-Path $env:LOCALAPPDATA "Programs\Earth"
    if (Test-Path (Join-Path $old "earth.exe")) {
        Remove-Item -Path $old -Recurse -Force -ErrorAction SilentlyContinue
        $path = [Environment]::GetEnvironmentVariable("Path", "User")
        if ($path) {
            $kept = $path.Split(";") | Where-Object { $_ -and ($_.TrimEnd("\") -ne $old) }
            [Environment]::SetEnvironmentVariable("Path", ($kept -join ";"), "User")
        }
        Remove-Item -Path "HKCU:\Software\Classes\Earth.Program" -Recurse -Force -ErrorAction SilentlyContinue
    }
}

if ($Uninstall) {
    $entry = Get-ItemProperty -Path $UninstallKey -ErrorAction SilentlyContinue
    if ($entry) {
        $uninstaller = $entry.UninstallString.Trim('"')
        Start-Process -FilePath $uninstaller -ArgumentList "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART" -Wait
    }
    Remove-OldInstall
    Say "Earth Studio kaldırıldı." "Earth Studio was uninstalled."
    return
}

# Windows PowerShell 5.1 defaults to old TLS versions that GitHub refuses.
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

if ($BaseUrl) {
    $base = $BaseUrl.TrimEnd("/")
} elseif ($Version) {
    $base = "https://github.com/$Repository/releases/download/$Version"
} else {
    $base = "https://github.com/$Repository/releases/latest/download"
}

$work = Join-Path ([IO.Path]::GetTempPath()) ("earth-install-" + [Guid]::NewGuid())
New-Item -ItemType Directory -Path $work | Out-Null
try {
    $setup = Join-Path $work $Asset
    Say "Earth Studio indiriliyor: $base/$Asset" "Downloading Earth Studio: $base/$Asset"
    Invoke-WebRequest -UseBasicParsing -Uri "$base/$Asset" -OutFile $setup

    # Verify the download against the checksums published with the release.
    $sums = Join-Path $work "SHA256SUMS.txt"
    Invoke-WebRequest -UseBasicParsing -Uri "$base/SHA256SUMS.txt" -OutFile $sums
    $line = Get-Content $sums | Where-Object { $_ -match "\s\*?$([regex]::Escape($Asset))$" } | Select-Object -First 1
    if (-not $line) {
        Fail "SHA256SUMS.txt içinde $Asset bulunamadı." "$Asset is missing from SHA256SUMS.txt."
    }
    $expected = ($line -split "\s+")[0].ToLower()
    $actual = (Get-FileHash -Algorithm SHA256 -Path $setup).Hash.ToLower()
    if ($expected -ne $actual) {
        Fail "İndirilen dosya doğrulanamadı (SHA-256 uyuşmuyor). Kurulum durduruldu." "The download failed verification (SHA-256 mismatch). Installation stopped."
    }
    Say "SHA-256 doğrulandı: $actual" "SHA-256 verified: $actual"

    Remove-OldInstall
    Say "Kuruluyor..." "Installing..."
    $language = if ($Turkish) { "turkish" } else { "english" }
    $process = Start-Process -FilePath $setup -Wait -PassThru -ArgumentList @(
        "/SILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CURRENTUSER",
        "/TASKS=desktopicon", "/LANG=$language"
    )
    if ($process.ExitCode -ne 0) {
        Fail "Kurulum başarısız oldu (çıkış kodu $($process.ExitCode))." "Setup failed (exit code $($process.ExitCode))."
    }

    Say "" ""
    Say "Earth Studio kuruldu. Masaüstündeki ya da Başlat menüsündeki 'Earth Studio' ile açabilirsiniz." "Earth Studio was installed. Open it from 'Earth Studio' on the desktop or in the Start menu."
} finally {
    Remove-Item -Path $work -Recurse -Force -ErrorAction SilentlyContinue
}
