# Rebuild the bundled startup audio from the owner's recordings. Originals stay untouched.
$ErrorActionPreference = 'Stop'
$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$recordings = Join-Path $repoRoot 'audio files'
$assets = Join-Path $repoRoot 'src\assets\sounds'
$sources = [ordered]@{
    konami = 'konami.mp3'
    rockstar = 'rdr2_shotgun_blast.mp3'
    gba = 'game-boy-advance-sp-intro.mp3'
    gamecube = 'gamecube intro.mp3'
    ps1 = 'psx.mp3'
    ps2 = 'ps2-nostalgic-startup.mp3'
    ps3 = 'ps3boot.mp3'
    psp = 'psp part 1.mp3', 'psp part 2.mp3'
    xbox360 = 'xbox-360-startup.mp3'
    switch = 'switch-sound.mp3'
}

# Check every source before replacing any bundled asset.
foreach ($source in $sources.Values) {
    foreach ($file in $source) {
        if (-not (Test-Path -LiteralPath (Join-Path $recordings $file) -PathType Leaf)) {
            throw "Missing recording: $file"
        }
    }
}
foreach ($key in $sources.Keys) {
    $ffArgs = @('-hide_banner', '-loglevel', 'error', '-y')
    foreach ($file in $sources[$key]) {
        $ffArgs += @('-i', (Join-Path $recordings $file))
    }
    if ($key -eq 'psp') {
        $ffArgs += @('-filter_complex', '[0:a][1:a]concat=n=2:v=0:a=1[out]', '-map', '[out]')
    }
    $ffArgs += @('-vn', '-map_metadata', '-1', '-ar', '48000', '-ac', '2', '-c:a', 'libopus', '-b:a', '96k', (Join-Path $assets "intro_$key.ogg"))
    & ffmpeg @ffArgs
    if ($LASTEXITCODE -ne 0) { throw "Converting $key failed ($LASTEXITCODE)" }
    Write-Output "Updated intro_$key.ogg"
}
