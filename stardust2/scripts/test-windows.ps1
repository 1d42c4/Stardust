param([switch]$Offline, [ValidateRange(1, 64)][int]$TestThreads = 2)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$reportDirectory = Join-Path $projectRoot 'target\validation'
[IO.Directory]::CreateDirectory($reportDirectory) | Out-Null
$reportPath = Join-Path $reportDirectory 'windows.log'
$oldTarget = $env:CARGO_TARGET_DIR
$env:CARGO_TARGET_DIR = Join-Path $projectRoot 'target'
$cargoOptions = @('--locked')
if ($Offline) { $cargoOptions += '--offline' }
function Invoke-Cargo([string[]]$CargoArguments) {
    # Windows PowerShell 5 wraps native stderr in ErrorRecord even for normal
    # compiler progress. Capture it as text; the native exit code decides failure.
    $previousPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        & cargo @CargoArguments 2>&1 | ForEach-Object { "$_" } | Tee-Object -FilePath $reportPath -Append
        $cargoExitCode = $LASTEXITCODE
    } finally { $ErrorActionPreference = $previousPreference }
    if ($cargoExitCode -ne 0) { throw "cargo failed (exit ${cargoExitCode}): $CargoArguments" }
}
Push-Location -LiteralPath $projectRoot
try {
    "Windows validation started: $([DateTime]::UtcNow.ToString('o'))" | Tee-Object -FilePath $reportPath
    & rustc --version --verbose | Tee-Object -FilePath $reportPath -Append
    if ($LASTEXITCODE -ne 0) { throw 'Rust toolchain unavailable' }
    Invoke-Cargo -CargoArguments @('fmt', '--all', '--', '--check')
    Invoke-Cargo -CargoArguments (@('test', '--release') + $cargoOptions + @('--', '--test-threads', "$TestThreads"))
    Invoke-Cargo -CargoArguments (@('clippy', '--all-targets') + $cargoOptions + @('--', '-D', 'warnings'))
    Invoke-Cargo -CargoArguments (@('build', '--release') + $cargoOptions)
    'Windows validation PASSED.' | Tee-Object -FilePath $reportPath -Append
} finally {
    $env:CARGO_TARGET_DIR = $oldTarget
    Pop-Location
}
