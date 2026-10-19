# Builds a signed KaneegaBell release and (optionally) publishes it to GitHub.
#
#   powershell -ExecutionPolicy Bypass -File tools\release\release.ps1 -Version 0.3.0
#   (the changelog is the "### 0.3.0" lines under "## Patch notes" in README.md)
#   ... -Notes "What changed"        (changelog given here instead)
#   ... -NotesFile notes.md          (changelog from a file instead)
#   ... -Publish                     (also push and publish the release; asks first)
#   ... -Repoint                     (only rewrite the updates branch for a published version)
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
    [switch]$Publish,
    # Only (re)write the updates branch for an already published version.
    [switch]$Repoint
)
$ErrorActionPreference = "Stop"
if ($Repoint) { $Publish = $true }
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
if (-not (git tag --list "v$Version") -and (git status --porcelain)) { Fail "commit or stash your changes first (the release commit should hold only the version bump)" }
# Already built and tagged (e.g. publishing was declined last time): with
# -Publish, publish those files without building again.
$publishOnly = $false
if (git tag --list "v$Version") {
    $built = Join-Path $root "release\v$Version\KaneegaBell_${Version}_x64-setup.exe"
    if (-not ($Publish -and (Test-Path $built) -and (Test-Path "$built.sig"))) { Fail "tag v$Version already exists" }
    $publishOnly = $true
    Write-Host "v$Version is already built and tagged: publishing those files."
}
$current = (& node -e "console.log(require('./src-tauri/tauri.conf.json').version)").Trim()
$newer = (& node --input-type=module -e "import { compareVersions } from './tools/release/set-version.mjs'; console.log(compareVersions('$Version', '$current') > 0)").Trim()
# The current version may be released once (its first release), else it must be newer.
if (-not $publishOnly -and $newer -ne "true" -and $Version -ne $current) { Fail "v$Version isn't newer than the current $current" }
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
elseif ($publishOnly -and (Test-Path $notesPath)) { }
else {
    # Usually: the version's lines in the README's "Patch notes" section.
    & node tools\release\notes.mjs $Version $notesPath
    if ($LASTEXITCODE -ne 0) { Fail "add a '### $Version' heading with one '- ' line per change under '## Patch notes' in README.md (or use -Notes / -NotesFile)" }
}
$installerName = "KaneegaBell_${Version}_x64-setup.exe"
$installerOut = Join-Path $out $installerName

if (-not $publishOnly) {
Write-Host "Releasing $current -> $Version"

Step "Version"
Run node @("tools\release\set-version.mjs", $Version)

Step "Tests"
Push-Location src-tauri
# The tests make temp folders: keep them in target\ (git-ignored), because security software
# can refuse new programs creating folders under AppData (seen 2026-10-19).
$oldTemp = $env:TEMP; $oldTmp = $env:TMP
$env:TEMP = $env:TMP = (New-Item -ItemType Directory -Force "target\tmp").FullName
& cargo test --lib --quiet
$testExit = $LASTEXITCODE
$env:TEMP = $oldTemp; $env:TMP = $oldTmp
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

$installer = Join-Path $root "src-tauri\target\release\bundle\nsis\$installerName"
if (-not (Test-Path $installer) -or -not (Test-Path "$installer.sig")) { Fail "installer or signature missing: $installer(.sig)" }
Copy-Item $installer, "$installer.sig" $out -Force
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
}

if (-not $Publish) {
    Write-Host "`nNot published. To publish these files later, run the same command with -Publish (it won't build again)."
    exit 0
}

# Puts the installer and latest.json on the repo's "updates" branch: what
# installed copies read. KB sends "Accept: application/vnd.github.raw+json" and
# its access code for both the manifest and the installer, and GitHub's
# contents API answers that with the file's raw bytes, public repo or private.
# (A release asset link doesn't work for that header: it returns the asset's
# description, which KB's signature check rightly refuses.)
function Update-UpdatesBranch {
    Step "Updates branch (what installed copies read)"
    Run git @("fetch", "-q", "origin", "updates")
    $wt = Join-Path ([IO.Path]::GetTempPath()) "kb-updates-$Version"
    Quiet git @("worktree", "remove", "--force", $wt) | Out-Null
    if (Test-Path -LiteralPath $wt) { Fail "a leftover folder is in the way: $wt" }
    Run git @("worktree", "add", "-q", "--detach", $wt, "origin/updates")
    try {
        # Only the newest installer is kept on the branch.
        foreach ($old in Get-ChildItem -LiteralPath $wt -Filter "KaneegaBell_*_x64-setup.exe") {
            Run git @("-C", $wt, "rm", "-q", "--", $old.Name)
        }
        Copy-Item -LiteralPath $installerOut -Destination $wt
        Run node @("tools\release\latest-json.mjs", $Version, $installerOut, $notesPath, (Join-Path $wt "latest.json"), "--contents")
        Run git @("-C", $wt, "add", "-A")
        Run git @("-C", $wt, "commit", "-q", "-m", "KaneegaBell $Version")
        Run git @("-C", $wt, "push", "-q", "origin", "HEAD:updates")
    }
    finally {
        Quiet git @("worktree", "remove", "--force", $wt) | Out-Null
    }

    # Download both back exactly the way KB does, and compare.
    $token = (& gh auth token).Trim()
    $headers = @{ Authorization = "Bearer $token"; Accept = "application/vnd.github.raw+json"; "User-Agent" = "kb-release-check" }
    $base = "https://api.github.com/repos/$repo/contents"
    $ProgressPreference = "SilentlyContinue"
    $manifest = Invoke-RestMethod -Uri "$base/latest.json?ref=updates" -Headers $headers
    if ($manifest.version -ne $Version) { Fail "the updates branch shows $($manifest.version), not $Version" }
    $tmp = Join-Path ([IO.Path]::GetTempPath()) "kb-check-$installerName"
    Invoke-WebRequest -Uri $manifest.platforms.'windows-x86_64'.url -Headers $headers -OutFile $tmp -UseBasicParsing
    $same = (Get-FileHash -LiteralPath $tmp).Hash -eq (Get-FileHash -LiteralPath $installerOut).Hash
    [IO.File]::Delete($tmp)
    if (-not $same) { Fail "the installer KB would download doesn't match the release's" }
    Write-Host "Checked: installed copies download exactly this installer." -ForegroundColor Green
}

if ($Repoint) {
    Update-UpdatesBranch
    exit 0
}

Step "Publish"
$answer = Read-Host "Publish KaneegaBell $Version on github.com/${repo}? Everyone with KB will be offered it. (yes/no)"
if ($answer.Trim() -notmatch '^(y|yes)$') { Write-Host "Not published. The commit and tag stay local; run the same command again to publish (it won't build again)."; exit 0 }
Run git @("push", "origin", "main", "v$Version")
# The release page (installer and .sig, for downloading by hand).
Run gh @("release", "create", "v$Version", "--repo", $repo, "--title", "KaneegaBell $Version", "--notes-file", $notesPath, "--latest",
    $installerOut, "$installerOut.sig", (Join-Path $out "latest.json"))
Update-UpdatesBranch
Write-Host "`nPublished: https://github.com/$repo/releases/tag/v$Version (installed copies will be offered it)" -ForegroundColor Green