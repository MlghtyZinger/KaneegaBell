# Embedded read-only inventory helper. Provider is chosen by Rust's fixed allowlist.
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
$rows = [System.Collections.Generic.List[object]]::new()
$warnings = [System.Collections.Generic.List[string]]::new()
$client = $null
$complete = $true
function Value($key, $name) {
    if (Test-Path -LiteralPath $key) { return (Get-Item -LiteralPath $key).GetValue($name, $null) }
    return $null
}
function Children($key) {
    if (Test-Path -LiteralPath $key) { return @(Get-ChildItem -LiteralPath $key) }
    return @()
}
function AddGame($id, $name, $dir, $exes) {
    if (-not $id -or -not $dir) { return }
    if ($rows.Count -ge 4096) { throw 'Too many installed-game records' }
    $rows.Add(@{id=[string]$id;name=[string]$name;dir=[string]$dir;exes=@($exes)})
}
switch ($Provider) {
 'gog' {
    $base='Registry::HKEY_LOCAL_MACHINE\SOFTWARE\WOW6432Node\GOG.com'
    $root=Value "$base\GalaxyClient\paths" 'client'
    if ($root) { $client=Join-Path $root 'GalaxyClient.exe' }
    $keys=@("$base\Games",'Registry::HKEY_LOCAL_MACHINE\SOFTWARE\GOG.com\Games')
    if (-not ($keys | Where-Object { Test-Path -LiteralPath $_ })) { $complete=$false; $warnings.Add('GOG installed-game registry is unavailable.') }
    foreach($key in $keys){ foreach($game in (Children $key)){ AddGame $game.PSChildName (Value $game.PSPath 'gameName') (Value $game.PSPath 'path') @() } }
 }
 'ubisoft' {
    $keys=@('Registry::HKEY_LOCAL_MACHINE\SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs','Registry::HKEY_LOCAL_MACHINE\SOFTWARE\Ubisoft\Launcher\Installs')
    if (-not ($keys | Where-Object { Test-Path -LiteralPath $_ })) { $complete=$false; $warnings.Add('Ubisoft installed-game registry is unavailable.') }
    foreach($key in $keys){foreach($game in (Children $key)){ $dir=Value $game.PSPath 'InstallDir'; if($dir){AddGame $game.PSChildName ([IO.Path]::GetFileName($dir.TrimEnd('\','/'))) $dir @()} }}
 }
 'battlenet' {
    foreach($key in @('Registry::HKEY_LOCAL_MACHINE\SOFTWARE\WOW6432Node\Blizzard Entertainment\Battle.net','Registry::HKEY_LOCAL_MACHINE\SOFTWARE\Blizzard Entertainment\Battle.net')){
      $root=Value $key 'InstallPath';if($root){$client=Join-Path $root 'Battle.net.exe'}
    }
    if(-not $client){$client=Join-Path ${env:ProgramFiles(x86)} 'Battle.net/Battle.net.exe'}
 }
 'ea' {
    $roots=@('Registry::HKEY_LOCAL_MACHINE\SOFTWARE\EA Games','Registry::HKEY_LOCAL_MACHINE\SOFTWARE\WOW6432Node\EA Games','Registry::HKEY_LOCAL_MACHINE\SOFTWARE\Electronic Arts\EA Games','Registry::HKEY_LOCAL_MACHINE\SOFTWARE\WOW6432Node\Electronic Arts\EA Games')
    $found=$false
    foreach($root in $roots){if(Test-Path -LiteralPath $root){$found=$true};foreach($game in (Children $root)){
      $dir=$null;foreach($field in @('Install Dir','InstallDir','InstallLocation')){if(-not $dir){$dir=Value $game.PSPath $field}}
      if($dir){AddGame $game.PSChildName $game.PSChildName $dir @()}
    }}
    if(-not $found -and -not (Test-Path -LiteralPath (Join-Path $env:PROGRAMDATA 'EA Desktop/InstallData'))){$complete=$false}
 }
 'xboxpc' {
    $known=@((Children 'Registry::HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\GamingServices\PackageRepository\Root') | ForEach-Object {$_.PSChildName})
    foreach($pkg in (Get-AppxPackage -PackageTypeFilter Main)){
      $dir=$pkg.InstallLocation
      if(-not $dir){continue}
      $config=Join-Path $dir 'MicrosoftGame.config'
      if($known -notcontains $pkg.PackageFamilyName -and -not (Test-Path -LiteralPath $config)){continue}
      try {
        $manifest=Get-AppxPackageManifest -Package $pkg.PackageFullName
        foreach($app in @($manifest.Package.Applications.Application)){
          if(-not $app.Id){continue}
          $exe=[string]$app.Executable
          AddGame ($pkg.PackageFamilyName+'!'+$app.Id) ([string]$manifest.Package.Properties.DisplayName) $dir @($exe)
        }
      } catch { $complete=$false; $warnings.Add('An Xbox package manifest was unreadable; previous games were kept.') }
    }
 }
 default { throw 'Unknown inventory provider' }
}
@{client=$client;games=@($rows.ToArray());warnings=@($warnings.ToArray());complete=$complete} | ConvertTo-Json -Depth 6 -Compress
