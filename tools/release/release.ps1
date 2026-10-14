# Builds a signed KaneegaBell release and (optionally) publishes it to GitHub.
#
#   powershell -ExecutionPolicy Bypass -File tools\release\release.ps1 -Version 0.3.0 -Notes "What changed"
#   ... -NotesFile notes.md          (changelog from a file instead)
#   ... -Publish                     (also push and publish the release; asks first)
#
# Steps: checks the work tree is clean -> sets the version everywhere -> runs
# the tests -> builds the installer signed with the updater key (asks for its
# password) -> commits "Release vX.Y.Z" + tag -> with -Publish and your OK:
# pushes, creates the GitHub release with the installer and its signature, and
# updates latest.json on the repo's "updates" branch, which installed copies
# read (with their update access code once the repo is private). Publishing
# uses GitHub's gh tool, signed in once with "gh auth login".
#
# The finished files stay in release\vX.Y.Z\ (installer, .sig, latest.json).
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
$repo = "MlghtyZinger/KaneegaBell"

function Fail($msg) { Write-Host "release: $msg" -ForegroundColor Red; exit 1 }
function Step($msg) { Write-Host "`n== $msg" -ForegroundColor Cyan }
# Runs a native command with its error output hidden and returns its exit code
# (stdout goes to $script:QuietOut). Windows PowerShell 5.1 turns redirected
# native error output into exceptions under "Stop", so relax that meanwhile.
function Quiet($exe, [string[]]$argList) {
    $old = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try { $script:QuietOut = & $exe @argList 2>$null; return $LASTEXITCODE }
    finally { $ErrorActionPreference = $old }
}
function Run($exe, [string[]]$argList) {
    & $exe @argList
    if ($LASTEXITCODE -ne 0) { Fail "$exe $($argList -join ' ') failed (exit $LASTEXITCODE)" }
}

# A PATH entry with a stray quote breaks npm/cmd; drop such entries and make
# sure Node and gh are reachable (this process only).
$env:Path = (($env:Path -split ';') | Where-Object { $_ -and $_ -notmatch '"' }) -join ';'
foreach ($dir in "C:\Program Files\nodejs", "C:\Program Files\GitHub CLI") {
    if ((Test-Path $dir) -and -not (($env:Path -split ';') -contains $dir)) { $env:Path += ";$dir" }
}
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
if ($Publish) {
    # Before the password and the build, not after.
    if (-not (Get-Command gh -ErrorAction SilentlyContinue)) { Fail "publishing needs GitHub's gh tool (winget install GitHub.cli), then 'gh auth login' once" }
    if ((Quiet gh @("auth", "status")) -ne 0) { Fail "gh isn't signed in: run 'gh auth login' once (GitHub.com, HTTPS, log in with a web browser)" }
}

$out = Join-Path $root "release\v$Version"
New-Item -ItemType Directory -Force $out | Out-Null
# Kept beside the upload folder, so that folder holds only the release files.
$notesPath = Join-Path $root "release\v$Version-notes.md"
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
$installerOut = Join-Path $out $installerName
# The public-link manifest, attached to the release: what KB 0.2.2 and older
# read (only reachable while the repo is public).
Run node @("tools\release\latest-json.mjs", $Version, $installerOut, $notesPath, (Join-Path $out "latest.json"))

Step "Commit and tag"
Run git @("add", "src-tauri/tauri.conf.json", "src-tauri/Cargo.toml", "src-tauri/Cargo.lock", "package.json", "package-lock.json")
# Releasing the current version as-is leaves nothing to commit; the tag marks it.
git diff --cached --quiet
if ($LASTEXITCODE -ne 0) { Run git @("commit", "-q", "-m", "Release v$Version") }
Run git @("tag", "-a", "v$Version", "-m", "KaneegaBell $Version")
Write-Host "`nRelease files in $out" -ForegroundColor Green

if (-not $Publish) {
    Write-Host "`nNot published. To publish, delete the tag (git tag -d v$Version) and run again with -Publish."
    exit 0
}

Step "Publish"
$answer = Read-Host "Publish KaneegaBell $Version on github.com/$repo? Everyone with KB will be offered it. (yes/no)"
if ($answer -ne "yes") { Write-Host "Not published. The commit and tag stay local."; exit 0 }
Run git @("push", "origin", "main", "v$Version")
Run gh @("release", "create", "v$Version", "--repo", $repo, "--title", "KaneegaBell $Version", "--notes-file", $notesPath, "--latest",
    $installerOut, "$installerOut.sig", (Join-Path $out "latest.json"))

# The manifest KB 0.2.3+ reads: latest.json on the "updates" branch, with the
# installer's API link (readable with an access code once the repo is private).
$release = (& gh api "repos/$repo/releases/tags/v$Version") | ConvertFrom-Json
$assetId = ($release.assets | Where-Object { $_.name -eq $installerName }).id
if (-not $assetId) { Fail "the release has no $installerName asset" }
$apiManifest = Join-Path $out "latest-api.json"
Run node @("tools\release\latest-json.mjs", $Version, $installerOut, $notesPath, $apiManifest, "$assetId")

if ((Quiet gh @("api", "repos/$repo/git/ref/heads/updates")) -ne 0) {
    $mainSha = (git rev-parse HEAD).Trim()
    Run gh @("api", "-X", "POST", "repos/$repo/git/refs", "-f", "ref=refs/heads/updates", "-f", "sha=$mainSha")
}
$existing = $null
if ((Quiet gh @("api", "repos/$repo/contents/latest.json?ref=updates")) -eq 0) { $existing = ($script:QuietOut | ConvertFrom-Json).sha }
$content = [Convert]::ToBase64String([IO.File]::ReadAllBytes($apiManifest))
$put = @("api", "-X", "PUT", "repos/$repo/contents/latest.json", "-f", "message=KaneegaBell $Version", "-f", "branch=updates", "-f", "content=$content")
if ($existing) { $put += @("-f", "sha=$existing") }
Run gh ($put + @("--silent"))

# Read it back the way KB does.
$check = (& gh api -H "Accept: application/vnd.github.raw+json" "repos/$repo/contents/latest.json?ref=updates") | ConvertFrom-Json
if ($check.version -ne $Version) { Fail "the updates branch doesn't show $Version (got $($check.version))" }
Write-Host "`nPublished: https://github.com/$repo/releases/tag/v$Version (installed copies will be offered it)" -ForegroundColor Green
