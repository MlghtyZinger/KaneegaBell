# Builds a signed KaneegaBell release and (optionally) publishes it to GitHub.
#
#   powershell -ExecutionPolicy Bypass -File tools\release\release.ps1 -Version 0.3.0 -Notes "What changed"
#   ... -NotesFile notes.md          (changelog from a file instead)
#   ... -Publish                     (also push and create the GitHub release; asks first)
#
# Steps: checks the work tree is clean -> sets the version everywhere -> runs
# the tests -> builds the installer signed with the updater key (asks for its
# password) -> writes latest.json -> commits "Release vX.Y.Z" + tag ->
# with -Publish and your OK: pushes and creates the GitHub release (with the
# gh tool if it's installed and signed in, otherwise it tells you what to
# upload). The finished files are in release\vX.Y.Z\.
param(
    [Parameter(Mandatory = $true)][string]$Version,
    [string]$Notes,
    [string]$NotesFile,
    [string]$KeyPath = "$env:USERPROFILE\.tauri\kaneegabell.key",
    [switch]$Publish
)
$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
Set-Location $root
$repo = "SultanAlshehhi100/KaneegaBell"

function Fail($msg) { Write-Host "release: $msg" -ForegroundColor Red; exit 1 }
function Step($msg) { Write-Host "`n== $msg" -ForegroundColor Cyan }
function Run($exe, [string[]]$argList) {
    & $exe @argList
    if ($LASTEXITCODE -ne 0) { Fail "$exe $($argList -join ' ') failed (exit $LASTEXITCODE)" }
}

# A PATH entry with a stray quote breaks npm/cmd; drop such entries and make
# sure Node is reachable (this process only).
$env:Path = (($env:Path -split ';') | Where-Object { $_ -and $_ -notmatch '"' }) -join ';'
$nodeDir = "C:\Program Files\nodejs"
if (-not (Get-Command node -ErrorAction SilentlyContinue) -and (Test-Path "$nodeDir\node.exe")) { $env:Path += ";$nodeDir" }
if (-not (Get-Command node -ErrorAction SilentlyContinue)) { Fail "Node.js not found" }
$tauri = Join-Path $root "node_modules\@tauri-apps\cli\tauri.js"

Step "Checks"
if ($Version -notmatch '^\d+\.\d+\.\d+$') { Fail "version must look like 1.2.3" }
if (-not (Test-Path $KeyPath)) { Fail "updater signing key not found at $KeyPath" }
$branch = (git rev-parse --abbrev-ref HEAD).Trim()
if ($branch -ne "main") { Fail "releases are made from main (you're on $branch)" }
if (git status --porcelain) { Fail "commit or stash your changes first (the release commit should hold only the version bump)" }
if (git tag --list "v$Version") { Fail "tag v$Version already exists" }
$current = (& node -e "console.log(require('./src-tauri/tauri.conf.json').version)").Trim()
$newer = (& node --input-type=module -e "import { compareVersions } from './tools/release/set-version.mjs'; console.log(compareVersions('$Version', '$current') > 0)").Trim()
# The current version may be released once (its first release), else it must be newer.
if ($newer -ne "true" -and $Version -ne $current) { Fail "v$Version isn't newer than the current $current" }

$out = Join-Path $root "release\v$Version"
New-Item -ItemType Directory -Force $out | Out-Null
$notesPath = Join-Path $out "notes.md"
if ($NotesFile) { Copy-Item $NotesFile $notesPath -Force }
elseif ($Notes) { [IO.File]::WriteAllText($notesPath, $Notes + "`n", (New-Object Text.UTF8Encoding($false))) }
else { Fail "give the changelog with -Notes `"...`" or -NotesFile file.md" }
Write-Host "Releasing $current -> $Version"

Step "Version"
Run node @("tools\release\set-version.mjs", $Version)

Step "Tests"
Push-Location src-tauri
& cargo test --lib --quiet
$testExit = $LASTEXITCODE
Pop-Location
if ($testExit -ne 0) { git checkout -- .; Fail "Rust tests failed (version bump undone)" }
Run node @("--test", "tests/*.test.ts")

Step "Signed build (the updater key's password is asked for here, not stored)"
$secure = Read-Host "Updater key password" -AsSecureString
$env:TAURI_SIGNING_PRIVATE_KEY = $KeyPath
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = [Runtime.InteropServices.Marshal]::PtrToStringBSTR([Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure))
try {
    & node $tauri build --config src-tauri/tauri.release.conf.json
    $buildExit = $LASTEXITCODE
}
finally {
    Remove-Item env:TAURI_SIGNING_PRIVATE_KEY, env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
}
if ($buildExit -ne 0) { git checkout -- .; Fail "build failed (version bump undone)" }

$installerName = "KaneegaBell_${Version}_x64-setup.exe"
$installer = Join-Path $root "src-tauri\target\release\bundle\nsis\$installerName"
if (-not (Test-Path $installer) -or -not (Test-Path "$installer.sig")) { Fail "installer or signature missing: $installer(.sig)" }
Copy-Item $installer, "$installer.sig" $out -Force
Run node @("tools\release\latest-json.mjs", $Version, (Join-Path $out $installerName), $notesPath, (Join-Path $out "latest.json"))

Step "Commit and tag"
Run git @("add", "src-tauri/tauri.conf.json", "src-tauri/Cargo.toml", "src-tauri/Cargo.lock", "package.json", "package-lock.json")
# Releasing the current version as-is leaves nothing to commit; the tag marks it.
git diff --cached --quiet
if ($LASTEXITCODE -ne 0) { Run git @("commit", "-q", "-m", "Release v$Version") }
Run git @("tag", "-a", "v$Version", "-m", "KaneegaBell $Version")

$files = @((Join-Path $out $installerName), (Join-Path $out "$installerName.sig"), (Join-Path $out "latest.json"))
Write-Host "`nRelease files in $out" -ForegroundColor Green
$files | ForEach-Object { Write-Host "  $(Split-Path $_ -Leaf)" }

if (-not $Publish) {
    Write-Host "`nNot published. To publish: run again with -Publish (after deleting the tag: git tag -d v$Version), or push yourself:"
    Write-Host "  git push origin main v$Version"
    Write-Host "  then create a release for tag v$Version at https://github.com/$repo/releases/new and attach the three files above."
    exit 0
}

Step "Publish"
$answer = Read-Host "Publish KaneegaBell $Version publicly on github.com/$repo? Everyone with KB will be offered it. (yes/no)"
if ($answer -ne "yes") { Write-Host "Not published. The commit and tag stay local."; exit 0 }
Run git @("push", "origin", "main", "v$Version")
$gh = Get-Command gh -ErrorAction SilentlyContinue
if ($gh) {
    & gh auth status *> $null
    if ($LASTEXITCODE -eq 0) {
        Run gh (@("release", "create", "v$Version", "--repo", $repo, "--title", "KaneegaBell $Version", "--notes-file", $notesPath, "--latest") + $files)
        Write-Host "`nPublished: https://github.com/$repo/releases/tag/v$Version" -ForegroundColor Green
        exit 0
    }
}
Write-Host "`nPushed. Now create the release by hand:" -ForegroundColor Yellow
Write-Host "  1. Open https://github.com/$repo/releases/new?tag=v$Version"
Write-Host "  2. Title: KaneegaBell $Version; paste the notes from $notesPath"
Write-Host "  3. Attach the three files listed above, keep 'Set as the latest release' ticked, and publish."
