param()
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$projectRoot = Split-Path -Parent $PSScriptRoot
$utf8 = New-Object System.Text.UTF8Encoding($false)

function Encode-Html([string]$Value) {
    [System.Net.WebUtility]::HtmlEncode($Value)
}

function Format-Inline([string]$Value) {
    # A small, deliberately limited renderer for this repository's README.
    # Protect code spans before processing emphasis and links; all input is escaped.
    $spans = New-Object 'System.Collections.Generic.List[string]'
    $encoded = Encode-Html $Value
    $encoded = [regex]::Replace($encoded, '`([^`]+)`', {
        param($match)
        $token = "@@CODE$($spans.Count)@@"
        $spans.Add('<code>' + $match.Groups[1].Value + '</code>')
        $token
    })
    $encoded = [regex]::Replace($encoded, '\*\*(.+?)\*\*', '<strong>$1</strong>')
    $encoded = [regex]::Replace($encoded, '\[([^\]]+)\]\(([^)]+)\)', '<a href="$2">$1</a>')
    for ($index = 0; $index -lt $spans.Count; $index++) {
        $encoded = $encoded.Replace("@@CODE${index}@@", $spans[$index])
    }
    $encoded
}

function Convert-Readme([string]$Markdown) {
    $lines = $Markdown -split '\r?\n'
    $parts = New-Object 'System.Collections.Generic.List[string]'
    $index = 0
    while ($index -lt $lines.Count) {
        $line = $lines[$index]
        if ($line -match '^\s*$') { $index++; continue }
        if ($line -match '^```(.*)$') {
            $language = $matches[1]
            $code = New-Object 'System.Collections.Generic.List[string]'
            $index++
            while ($index -lt $lines.Count -and $lines[$index] -notmatch '^```\s*$') {
                $code.Add($lines[$index]); $index++
            }
            if ($index -ge $lines.Count) { throw 'Unclosed README code block' }
            $parts.Add('<pre><code data-language="' + (Encode-Html $language) + '">' +
                (Encode-Html ($code -join "`n")) + '</code></pre>')
            $index++; continue
        }
        if ($line -match '^(#{1,6})\s+(.+)$') {
            $level = $matches[1].Length
            $title = $matches[2]
            $id = ($title.ToLowerInvariant() -replace '[^a-z0-9]+', '-').Trim('-')
            $parts.Add("<h$level id=`"$id`">" + (Format-Inline $title) + "</h$level>")
            $index++; continue
        }
        if ($line.StartsWith('|')) {
            $parts.Add('<div class="table-scroll"><table class="readme-table">')
            $row = 0
            while ($index -lt $lines.Count -and $lines[$index].StartsWith('|')) {
                $cells = $lines[$index].Trim().Trim('|') -split '\|'
                if ($lines[$index] -notmatch '^\|[\s:|\-]+\|$') {
                    if ($row -eq 0) { $parts.Add('<thead>'); $tag = 'th' } else { $tag = 'td' }
                    $parts.Add('<tr>' + (($cells | ForEach-Object {
                        "<$tag>" + (Format-Inline $_.Trim()) + "</$tag>"
                    }) -join '') + '</tr>')
                    if ($row -eq 0) { $parts.Add('</thead><tbody>') }
                    $row++
                }
                $index++
            }
            $parts.Add('</tbody></table></div>'); continue
        }
        if ($line.StartsWith('- ')) {
            $parts.Add('<ul>')
            while ($index -lt $lines.Count -and $lines[$index].StartsWith('- ')) {
                $item = $lines[$index].Substring(2); $index++
                while ($index -lt $lines.Count -and $lines[$index] -match '^  \S') {
                    $item += ' ' + $lines[$index].Trim(); $index++
                }
                $parts.Add('<li>' + (Format-Inline $item) + '</li>')
            }
            $parts.Add('</ul>'); continue
        }
        $paragraph = $line; $index++
        while ($index -lt $lines.Count -and $lines[$index] -notmatch '^\s*$|^#|^```|^\||^- ') {
            $paragraph += ' ' + $lines[$index].Trim(); $index++
        }
        $parts.Add('<p>' + (Format-Inline $paragraph) + '</p>')
    }
    $parts -join "`n"
}

function Read-Validation([string]$Platform) {
    $path = Join-Path $projectRoot "target/validation/$($Platform.ToLowerInvariant()).log"
    $log = Get-Content -LiteralPath $path -Raw
    if ($log -notmatch "(?m)^$Platform validation PASSED\.\s*$") {
        throw "$Platform validation is missing or incomplete; run the platform script first."
    }
    $startedMatch = [regex]::Match($log, "(?m)^$Platform validation started: (.+)$")
    if (-not $startedMatch.Success) { throw "Missing $Platform run timestamp" }
    $started = [DateTimeOffset]::Parse($startedMatch.Groups[1].Value.Trim())
    $tests = @{}
    $group = ''
    foreach ($line in ($log -split '\r?\n')) {
        if ($line -match 'Running unittests src[\\/](lib|main)\.rs') { $group = $matches[1] }
        elseif ($line -match 'Running tests[\\/]([a-z_]+)\.rs') { $group = $matches[1] }
        elseif ($line -match 'Doc-tests stardust') { $group = 'doc' }
        elseif ($line -match '^test (.+) \.\.\. (ok|FAILED|ignored)(?:\s.*)?$') {
            if (-not $group) { throw "Test without a harness in $Platform log" }
            $name = $matches[1]; $status = $matches[2]
            $key = "$group/$name"
            if ($tests.ContainsKey($key)) { throw "Duplicate $Platform test: $key" }
            $tests[$key] = [pscustomobject]@{ group = $group; name = $name; status = $status }
        }
    }
    $summaries = [regex]::Matches($log, 'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;')
    if ($summaries.Count -ne 7) { throw "Expected all seven test harness summaries in $Platform log" }
    $passed = 0
    foreach ($summary in $summaries) {
        $passed += [int]$summary.Groups[1].Value
        foreach ($index in 2..5) {
            if ([int]$summary.Groups[$index].Value -ne 0) { throw "$Platform has failed, ignored, measured, or filtered tests" }
        }
    }
    if ($tests.Count -ne $passed -or @($tests.Values | Where-Object status -ne 'ok').Count) {
        throw "$Platform individual results disagree with the summaries"
    }
    [pscustomobject]@{ platform = $Platform; started = $started; tests = $tests; passed = $passed; log = $log }
}

function Get-TestSource([string]$Group, [string]$Name) {
    if ($Group -ne 'lib') {
        if ($Group -eq 'xor_cli' -and $Name.StartsWith('in_place::')) { return 'tests/cases/in_place.rs' }
        return "tests/$Group.rs"
    }
    switch -Regex ($Name) {
        '^keygen::cli::command_tests::' { return 'src/keygen/command_tests.rs' }
        '^keygen::cli::reliability_tests::' { return 'src/keygen/cli_reliability_tests.rs' }
        '^keygen::cli::tests::' { return 'src/keygen/cli_tests.rs' }
        '^keygen::storage::tests::' { return 'src/keygen/storage_tests.rs' }
        '^keygen::tests::' { return 'src/keygen/tests.rs' }
        '^xor::atomic_file::in_place_tests::' { return 'src/xor/tests/in_place.rs' }
        '^xor::atomic_file::tests::' { return 'src/xor/tests/transactions.rs' }
        '^xor::(arguments|file_checks|streams)::' { return "src/xor/tests/$($matches[1]).rs" }
        default { throw "No source mapping for $Name" }
    }
}

$windows = Read-Validation 'Windows'
$linux = Read-Validation 'Linux'
$sourceFiles = @('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml') | ForEach-Object { Get-Item -LiteralPath (Join-Path $projectRoot $_) }
$sourceFiles += Get-ChildItem -LiteralPath (Join-Path $projectRoot 'src'), (Join-Path $projectRoot 'tests') -Recurse -File
$earliestRun = @($windows.started, $linux.started) | Sort-Object | Select-Object -First 1
$changed = @($sourceFiles | Where-Object { $_.LastWriteTimeUtc -gt $earliestRun.UtcDateTime })
if ($changed.Count) { throw "Source changed after validation began; rerun both platform scripts: $($changed.FullName -join ', ')" }

$descriptions = @{}
foreach ($line in (Get-Content -LiteralPath (Join-Path $projectRoot 'docs/test-descriptions.txt') -Encoding UTF8)) {
    if (-not $line.Trim() -or $line.StartsWith('#')) { continue }
    $pair = $line -split '\|', 2
    if ($pair.Count -ne 2 -or $descriptions.ContainsKey($pair[0])) { throw "Invalid or duplicate description: $line" }
    $descriptions[$pair[0]] = $pair[1]
}
$allKeys = @(@($windows.tests.Keys) + @($linux.tests.Keys) | Sort-Object -Unique)
$rows = New-Object 'System.Collections.Generic.List[string]'
$records = New-Object 'System.Collections.Generic.List[object]'
foreach ($key in $allKeys) {
    $test = if ($windows.tests.ContainsKey($key)) { $windows.tests[$key] } else { $linux.tests[$key] }
    $leaf = ($test.name -split '::')[-1]
    if (-not $descriptions.ContainsKey($leaf)) { throw "Missing explanation for $key" }
    $source = Get-TestSource $test.group $test.name
    $winStatus = if ($windows.tests.ContainsKey($key)) { 'Passed' } else { 'Not included' }
    $linuxStatus = if ($linux.tests.ContainsKey($key)) { 'Passed' } else { 'Not included' }
    $records.Add([ordered]@{ harness = $test.group; test = $test.name; explanation = $descriptions[$leaf]; source = $source; windows = $winStatus; linux = $linuxStatus })
    $winClass = if ($winStatus -eq 'Passed') { 'pass' } else { 'neutral' }
    $linuxClass = if ($linuxStatus -eq 'Passed') { 'pass' } else { 'neutral' }
    $title = $leaf.Replace('_', ' ')
    $rows.Add('<tr data-group="' + $test.group + '" data-platform="' +
        $(if ($winStatus -eq 'Passed') { 'windows ' }) + $(if ($linuxStatus -eq 'Passed') { 'linux' }) + '">' +
        '<th scope="row"><span class="test-title">' + (Encode-Html $title) + '</span>' +
        '<details><summary>Test ID and source</summary><code>' + (Encode-Html $key) + '</code>' +
        '<a href="' + $source + '">' + $source + '</a></details></th>' +
        '<td>' + (Encode-Html $descriptions[$leaf]) + '</td>' +
        '<td><span class="badge ' + $winClass + '">' + $winStatus + '</span></td>' +
        '<td><span class="badge ' + $linuxClass + '">' + $linuxStatus + '</span></td></tr>')
}
$resultDirectory = Join-Path $projectRoot 'docs/test-results'
[IO.Directory]::CreateDirectory($resultDirectory) | Out-Null
foreach ($run in @($windows, $linux)) {
    [IO.File]::WriteAllText((Join-Path $resultDirectory "$($run.platform.ToLowerInvariant()).log"), $run.log, $utf8)
}
$hashes = @($sourceFiles | Sort-Object FullName | ForEach-Object {
    [ordered]@{ path = $_.FullName.Substring($projectRoot.Length + 1).Replace('\', '/'); sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() }
})
$data = [ordered]@{
    generated_utc = [DateTime]::UtcNow.ToString('o')
    windows_started_utc = $windows.started.ToString('o')
    linux_started_utc = $linux.started.ToString('o')
    windows_passed = $windows.passed; linux_passed = $linux.passed
    unique_tests = $records.Count; source_files = $hashes; tests = @($records.ToArray())
}
[IO.File]::WriteAllText((Join-Path $resultDirectory 'results.json'), ($data | ConvertTo-Json -Depth 6), $utf8)
$template = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'readme-report.template.html') -Raw -Encoding UTF8
$readme = Get-Content -LiteralPath (Join-Path $projectRoot 'README.md') -Raw -Encoding UTF8
$replacements = @{
    '{{README}}' = (Convert-Readme $readme)
    '{{ROWS}}' = ($rows -join "`n")
    '{{WINDOWS_COUNT}}' = [string]$windows.passed
    '{{LINUX_COUNT}}' = [string]$linux.passed
    '{{UNIQUE_COUNT}}' = [string]$records.Count
    '{{WINDOWS_DATE}}' = $windows.started.UtcDateTime.ToString('yyyy-MM-dd HH:mm:ss') + ' UTC'
    '{{LINUX_DATE}}' = $linux.started.UtcDateTime.ToString('yyyy-MM-dd HH:mm:ss') + ' UTC'
}
foreach ($key in $replacements.Keys) { $template = $template.Replace($key, $replacements[$key]) }
if ($template -match '\{\{[A-Z_]+\}\}') { throw 'Unresolved HTML placeholder' }
$htmlPath = Join-Path $projectRoot 'README.html'
[IO.File]::WriteAllText($htmlPath, $template, $utf8)
"Created $htmlPath with $($records.Count) explained tests: Windows $($windows.passed), Linux $($linux.passed)."
