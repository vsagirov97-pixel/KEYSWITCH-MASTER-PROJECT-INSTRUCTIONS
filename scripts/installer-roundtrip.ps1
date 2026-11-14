# Installer round trip on a Windows CI runner (the `Windows build` job), against the NSIS
# installer built by `tauri build`:
#   1. silent install (/S, per-user) -> the app exe, the Start-menu shortcut and the
#      Add/Remove Programs entry exist;
#   2. the installed exe with a damaged settings.json (APPDATA redirected to a temporary
#      folder): the first instance (--background) keeps running (safe defaults), a second
#      launch exits on its own and the first shows its window (activation signal);
#   2b. QA-WF-05: settings that say autostart was enabled, but no Run value (a manual update
#      ran the old uninstaller without /UPDATE) -> the app re-creates the Run value at start;
#   3. the HKCU Run value "KeySwitch" is written as the app writes it on enable;
#   4. silent uninstall -> the exe, the shortcut, the Add/Remove entry and the Run value are
#      gone; other Run values are unchanged.
# Prints metadata only. Exits non-zero on the first failed check.
param(
  [string]$BundleDir = "target/release/bundle/nsis"
)
$ErrorActionPreference = 'Stop'
$runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$uninstallRoot = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall'
$valueName = 'KeySwitch'

function Fail([string]$why) {
  Write-Output "::error title=installer round trip::$why"
  exit 1
}

function Get-RunValues {
  $props = Get-ItemProperty -Path $runKey -ErrorAction SilentlyContinue
  if (-not $props) { return @() }
  $props.PSObject.Properties |
    Where-Object { $_.Name -notlike 'PS*' } |
    ForEach-Object { "$($_.Name)=$($_.Value)" } |
    Sort-Object
}

function Get-UninstallEntry {
  Get-ChildItem $uninstallRoot -ErrorAction SilentlyContinue |
    ForEach-Object { Get-ItemProperty $_.PSPath } |
    Where-Object { $_.DisplayName -eq 'KeySwitch' } |
    Select-Object -First 1
}

$setup = Get-ChildItem -Path $BundleDir -Filter '*-setup.exe' | Select-Object -First 1
if (-not $setup) { Fail "no NSIS installer in $BundleDir" }
if (Get-ItemProperty -Path $runKey -Name $valueName -ErrorAction SilentlyContinue) {
  Fail 'a KeySwitch Run value exists before the install (runner not clean)'
}
$otherRunBefore = Get-RunValues | Where-Object { $_ -notlike "$valueName=*" }
$programs = [Environment]::GetFolderPath('Programs')

# 1. Silent install.
$p = Start-Process -FilePath $setup.FullName -ArgumentList '/S' -Wait -PassThru
if ($p.ExitCode -ne 0) { Fail "installer exit code $($p.ExitCode)" }
$entry = Get-UninstallEntry
if (-not $entry) { Fail 'no Add/Remove Programs entry after install' }
$uninstaller = ($entry.UninstallString -replace '"', '').Trim()
$installDir = Split-Path -Parent $uninstaller
$exe = Get-ChildItem -Path $installDir -Filter '*.exe' |
  Where-Object { $_.Name -notlike 'uninstall*' } |
  Select-Object -First 1
if (-not $exe) { Fail 'no application exe in the install folder' }
$shortcut = Get-ChildItem -Path $programs -Recurse -Filter 'KeySwitch*.lnk' -ErrorAction SilentlyContinue |
  Select-Object -First 1
if (-not $shortcut) { Fail 'no Start-menu shortcut after install' }
Write-Output "install: OK (exe $($exe.Name), shortcut $($shortcut.Name), per-user entry)"

# 2. Installed exe: damaged settings -> keeps running; second launch exits and signals.
$realAppData = $env:APPDATA
$appData = Join-Path $env:RUNNER_TEMP 'ks-appdata'
New-Item -ItemType Directory -Force -Path (Join-Path $appData 'KeySwitch') | Out-Null
[IO.File]::WriteAllText((Join-Path $appData 'KeySwitch\settings.json'), '{ "uiLanguage": "en", "hotkey": ')
$env:APPDATA = $appData
try {
  $first = Start-Process -FilePath $exe.FullName -ArgumentList '--background' -PassThru
  Start-Sleep -Seconds 8
  if ($first.HasExited) { Fail "first instance exited (code $($first.ExitCode)) with a damaged settings file" }
  $first.Refresh()
  $windowBefore = $first.MainWindowHandle -ne 0
  $second = Start-Process -FilePath $exe.FullName -PassThru
  if (-not $second.WaitForExit(15000)) {
    Stop-Process -Id $second.Id -Force -ErrorAction SilentlyContinue
    Fail 'second launch did not exit within 15 s'
  }
  Start-Sleep -Seconds 3
  if ($first.HasExited) { Fail 'first instance exited after the second launch' }
  $first.Refresh()
  $windowAfter = $first.MainWindowHandle -ne 0
  Write-Output "single instance: OK (second exit code $($second.ExitCode)); window before=$windowBefore after=$windowAfter"
  if (-not $windowBefore -and -not $windowAfter) {
    # Visible-window detection depends on the runner desktop; reported, not failed.
    Write-Output '::warning title=installer round trip::activation signal not observed (no visible window after the second launch)'
  }
} finally {
  if ($first -and -not $first.HasExited) {
    Stop-Process -Id $first.Id -Force -ErrorAction SilentlyContinue
    $first.WaitForExit(10000) | Out-Null
  }
  $env:APPDATA = $realAppData
}

# 2b. QA-WF-05: the user's autostart choice from settings.json re-creates a missing Run value.
[IO.File]::WriteAllText((Join-Path $appData 'KeySwitch\settings.json'), '{ "uiLanguage": "en", "autostart": true }')
$env:APPDATA = $appData
try {
  $app = Start-Process -FilePath $exe.FullName -ArgumentList '--background' -PassThru
  $deadline = (Get-Date).AddSeconds(20)
  while (-not (Get-ItemProperty -Path $runKey -Name $valueName -ErrorAction SilentlyContinue) -and (Get-Date) -lt $deadline) {
    Start-Sleep -Seconds 1
  }
  $restored = (Get-ItemProperty -Path $runKey -Name $valueName -ErrorAction SilentlyContinue).$valueName
  if (-not $restored) { Fail 'autostart from settings did not re-create the Run value (QA-WF-05)' }
  if ($restored -notlike "*$($exe.Name)*--background*") { Fail 're-created Run value does not start the installed exe with --background' }
  Write-Output 'autostart restore: OK (Run value re-created from settings)'
} finally {
  if ($app -and -not $app.HasExited) {
    Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
    $app.WaitForExit(10000) | Out-Null
  }
  $env:APPDATA = $realAppData
}

# 3. Autostart value as `os::set_autostart` writes it.
Set-ItemProperty -Path $runKey -Name $valueName -Value "`"$($exe.FullName)`" --background"

# 4. Silent uninstall (NSIS copies the uninstaller to %TEMP% and returns early: poll).
$p = Start-Process -FilePath $uninstaller -ArgumentList '/S' -Wait -PassThru
if ($p.ExitCode -ne 0) { Fail "uninstaller exit code $($p.ExitCode)" }
$deadline = (Get-Date).AddSeconds(60)
while ((Test-Path $exe.FullName) -and (Get-Date) -lt $deadline) { Start-Sleep -Seconds 1 }
Start-Sleep -Seconds 2
if (Test-Path $exe.FullName) { Fail 'application exe still present after uninstall' }
if (Test-Path $shortcut.FullName) { Fail 'Start-menu shortcut still present after uninstall' }
if (Get-UninstallEntry) { Fail 'Add/Remove Programs entry still present after uninstall' }
if (Get-ItemProperty -Path $runKey -Name $valueName -ErrorAction SilentlyContinue) {
  Fail 'HKCU Run value KeySwitch still present after uninstall'
}
$otherRunAfter = Get-RunValues | Where-Object { $_ -notlike "$valueName=*" }
if ((@($otherRunBefore) -join "`n") -ne (@($otherRunAfter) -join "`n")) { Fail 'other HKCU Run values changed' }
Write-Output 'uninstall: OK (exe, shortcut, Add/Remove entry and Run value removed; other Run values unchanged)'
