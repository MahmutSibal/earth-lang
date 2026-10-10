<#
.SYNOPSIS
    Earth programlama dilini kurar (Windows, yönetici izni gerekmez).
    Installs the Earth programming language (Windows, no administrator rights).

.DESCRIPTION
    - En son sürümü GitHub Releases'tan indirir ve SHA-256 özetini doğrular.
    - %LOCALAPPDATA%\Programs\Earth klasörüne kurar.
    - Kullanıcı PATH'ine ekler: yeni bir terminalde `earth` yazmak yeterli olur.
    - .earth dosyalarını Earth ile ilişkilendirir (çift tıklayınca çalışır).

    Kurulum / Install:
        irm https://raw.githubusercontent.com/MahmutSibal/earth-lang/main/install.ps1 | iex

    Kaldırma / Uninstall:
        & ([scriptblock]::Create((irm https://raw.githubusercontent.com/MahmutSibal/earth-lang/main/install.ps1))) -Uninstall
#>
param(
    # Kurulacak sürüm etiketi, örneğin v0.3.0. Boşsa en son sürüm.
    [string]$Version = "",
    # Kurulum klasörü.
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA "Programs\Earth"),
    # İndirmek yerine yerel bir zip dosyası kur (test ve çevrimdışı kurulum için).
    [string]$ZipPath = "",
    # Sürüm dosyalarının indirileceği adres (test için).
    [string]$BaseUrl = "",
    [switch]$Uninstall
)

$ErrorActionPreference = "Stop"
$Repository = "MahmutSibal/earth-lang"
$Asset = "earth-windows-x64.zip"
$OnWindows = $env:OS -eq "Windows_NT"
$Turkish = (Get-Culture).TwoLetterISOLanguageName -eq "tr"

function Say([string]$Tr, [string]$En) {
    if ($Turkish) { Write-Host $Tr } else { Write-Host $En }
}

function Fail([string]$Tr, [string]$En) {
    if ($Turkish) { throw $Tr } else { throw $En }
}

function Get-UserPath {
    $path = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($null -eq $path) { return "" }
    return $path
}

function Set-FileAssociation([string]$Exe) {
    $classes = "HKCU:\Software\Classes"
    New-Item -Path "$classes\.earth" -Force | Out-Null
    Set-ItemProperty -Path "$classes\.earth" -Name "(default)" -Value "Earth.Program"
    New-Item -Path "$classes\Earth.Program\DefaultIcon" -Force | Out-Null
    Set-ItemProperty -Path "$classes\Earth.Program" -Name "(default)" -Value "Earth"
    Set-ItemProperty -Path "$classes\Earth.Program\DefaultIcon" -Name "(default)" -Value "`"$Exe`",0"
    New-Item -Path "$classes\Earth.Program\shell\open\command" -Force | Out-Null
    Set-ItemProperty -Path "$classes\Earth.Program\shell\open\command" -Name "(default)" -Value "`"$Exe`" `"%1`""
}

function Remove-FileAssociation {
    Remove-Item -Path "HKCU:\Software\Classes\.earth" -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item -Path "HKCU:\Software\Classes\Earth.Program" -Recurse -Force -ErrorAction SilentlyContinue
}

if ($Uninstall) {
    if (Test-Path $InstallDir) { Remove-Item -Path $InstallDir -Recurse -Force }
    if ($OnWindows) {
        $remaining = (Get-UserPath).Split(";") | Where-Object { $_ -and ($_.TrimEnd("\") -ne $InstallDir.TrimEnd("\")) }
        [Environment]::SetEnvironmentVariable("Path", ($remaining -join ";"), "User")
        Remove-FileAssociation
    }
    Say "Earth kaldırıldı." "Earth was uninstalled."
    return
}

# Windows PowerShell 5.1 defaults to old TLS versions that GitHub refuses.
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$work = Join-Path ([IO.Path]::GetTempPath()) ("earth-install-" + [Guid]::NewGuid())
New-Item -ItemType Directory -Path $work | Out-Null
try {
    if ($ZipPath) {
        $zip = (Resolve-Path $ZipPath).Path
    } else {
        if ($BaseUrl) {
            $base = $BaseUrl.TrimEnd("/")
        } elseif ($Version) {
            $base = "https://github.com/$Repository/releases/download/$Version"
        } else {
            $base = "https://github.com/$Repository/releases/latest/download"
        }
        $zip = Join-Path $work $Asset
        Say "Earth indiriliyor: $base/$Asset" "Downloading Earth: $base/$Asset"
        Invoke-WebRequest -UseBasicParsing -Uri "$base/$Asset" -OutFile $zip

        # Verify the download against the checksums published with the release.
        $sums = Join-Path $work "SHA256SUMS.txt"
        try {
            Invoke-WebRequest -UseBasicParsing -Uri "$base/SHA256SUMS.txt" -OutFile $sums
        } catch {
            $sums = $null
            Say "Uyarı: bu sürümde SHA256SUMS.txt yok, doğrulama atlandı." "Warning: this release has no SHA256SUMS.txt; skipping verification."
        }
        if ($sums) {
            $line = Get-Content $sums | Where-Object { $_ -match "\s\*?$([regex]::Escape($Asset))$" } | Select-Object -First 1
            if (-not $line) {
                Fail "SHA256SUMS.txt içinde $Asset bulunamadı." "$Asset is missing from SHA256SUMS.txt."
            }
            $expected = ($line -split "\s+")[0].ToLower()
            $actual = (Get-FileHash -Algorithm SHA256 -Path $zip).Hash.ToLower()
            if ($expected -ne $actual) {
                Fail "İndirilen dosya doğrulanamadı (SHA-256 uyuşmuyor). Kurulum durduruldu." "The download failed verification (SHA-256 mismatch). Installation stopped."
            }
            Say "SHA-256 doğrulandı: $actual" "SHA-256 verified: $actual"
        }
    }

    $extracted = Join-Path $work "extracted"
    Expand-Archive -Path $zip -DestinationPath $extracted -Force
    $exe = Get-ChildItem -Path $extracted -Recurse -Filter "earth.exe" | Select-Object -First 1
    if (-not $exe) {
        Fail "Arşivde earth.exe bulunamadı." "earth.exe was not found in the archive."
    }

    if (Test-Path $InstallDir) { Remove-Item -Path $InstallDir -Recurse -Force }
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Copy-Item -Path (Join-Path $exe.Directory.FullName "*") -Destination $InstallDir -Recurse -Force
    $installed = Join-Path $InstallDir "earth.exe"

    if ($OnWindows) {
        Get-ChildItem -Path $InstallDir -Recurse -File | Unblock-File
        $userPath = Get-UserPath
        $entries = $userPath.Split(";") | Where-Object { $_ }
        if (-not ($entries | Where-Object { $_.TrimEnd("\") -eq $InstallDir.TrimEnd("\") })) {
            [Environment]::SetEnvironmentVariable("Path", (($entries + $InstallDir) -join ";"), "User")
        }
        if (-not (($env:Path.Split(";")) -contains $InstallDir)) { $env:Path = "$env:Path;$InstallDir" }
        Set-FileAssociation $installed
    }

    Say "" ""
    Say "Earth kuruldu: $installed" "Earth was installed: $installed"
    Say "Yeni bir terminal açıp 'earth' yazın ya da bir .earth dosyasına çift tıklayın." "Open a new terminal and type 'earth', or double-click a .earth file."
} finally {
    Remove-Item -Path $work -Recurse -Force -ErrorAction SilentlyContinue
}
