param(
    [Parameter(Mandatory = $true)]
    [string]$FixtureRoot,
    [string]$Output = "",
    [int]$BudgetMs = 120000
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$fixtureRoot = (Resolve-Path $FixtureRoot).Path
$workspace = Join-Path $fixtureRoot "release-smoke"
$ymap = Join-Path $workspace "simple.ymap"

if (-not (Test-Path -LiteralPath $ymap -PathType Leaf)) {
    throw "Synthetic release-smoke YMAP not found: $ymap"
}

if ([string]::IsNullOrWhiteSpace($Output)) {
    $Output = Join-Path $root "release-smoke-report.json"
} elseif (-not [System.IO.Path]::IsPathRooted($Output)) {
    $Output = Join-Path $root $Output
}

$specPath = Join-Path ([System.IO.Path]::GetTempPath()) ("ragelab-studio-release-smoke-" + [guid]::NewGuid().ToString("N") + ".json")
$spec = [ordered]@{
    output = $Output
    scene = [ordered]@{
        workspace = $workspace
        ymap = $ymap
        fallbackRoots = @()
        rpfMounts = @()
        gameIndex = $null
        maxNodes = 128
    }
    width = 640
    height = 360
    collision = $false
}
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($specPath, ($spec | ConvertTo-Json -Depth 8), $utf8NoBom)

try {
    Push-Location $root
    try {
        & bun run build
        if ($LASTEXITCODE -ne 0) {
            throw "Studio frontend release build failed with exit code $LASTEXITCODE"
        }

        & cargo build --release --manifest-path src-tauri/Cargo.toml
        if ($LASTEXITCODE -ne 0) {
            throw "Studio Rust release build failed with exit code $LASTEXITCODE"
        }

        $exe = Join-Path $root "src-tauri\target\release\ragelab-studio.exe"
        if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
            throw "Standalone release executable not found: $exe"
        }

        $started = [Diagnostics.Stopwatch]::StartNew()
        $process = Start-Process -FilePath $exe -ArgumentList @("--native-viewport-smoke", $specPath) -Wait -PassThru
        $exitCode = $process.ExitCode
        $started.Stop()
        if ($exitCode -ne 0) {
            throw "Standalone release smoke failed with exit code $exitCode"
        }
        if ($started.ElapsedMilliseconds -gt $BudgetMs) {
            throw "Standalone release smoke exceeded budget: $($started.ElapsedMilliseconds) ms > $BudgetMs ms"
        }

        if (-not (Test-Path -LiteralPath $Output -PathType Leaf)) {
            throw "Standalone release smoke report was not written: $Output"
        }
        $report = Get-Content -Raw -LiteralPath $Output | ConvertFrom-Json
        if ($report.ok -ne $true) {
            throw "Standalone release smoke report did not return ok=true"
        }

        [ordered]@{
            schema = "ragelab.studio.release-smoke"
            schemaVersion = 1
            ok = $true
            elapsedMs = $started.ElapsedMilliseconds
            budgetMs = $BudgetMs
            executable = $exe
            report = $Output
            installerBuilt = $false
        } | ConvertTo-Json -Compress
    } finally {
        Pop-Location
    }
} finally {
    Remove-Item -Force -LiteralPath $specPath -ErrorAction SilentlyContinue
}
