param(
    # sweep baseline directory to diff against (.scratch/sweep34 was the RC
    # baseline as of 2026-09-22); the sweep itself is only run when this is set
    [string]$Baseline = "",
    [string]$OutDir = "",
    # the two installer gates need Inno Setup and rebuild dist/; skip them for
    # a quick pass and run them separately before calling a head converged
    [switch]$SkipInstallers
)

# T4.4 · the RC gates, as one command. Each gate is the same shape as its
# row in docs/REPORT_TRACK4.md Slice 0: one command, one expected output.
# Nothing here deletes anything (the delete guard turned that into a failure
# mode — see Slice 1) and every run writes its own timestamped log, so a
# log's content is always the run that named it.
#
# Usage:
#   benchmarks/scripts/rc_gates.ps1 -Baseline .scratch/sweep34
# The script does not judge pixels; sweep diffs and bbox tables stay with
# diffbbox.ps1 and the human, as everywhere else.

$ErrorActionPreference = "Continue"
$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
if ($OutDir -eq "") { $OutDir = ".scratch/t4-rc/gates-$stamp" }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$log = Join-Path $OutDir "gates.log"
$rows = @()

function Record([string]$gate, [bool]$ok, [string]$note) {
    $script:rows += [pscustomobject]@{ gate = $gate; ok = $ok; note = $note }
    Write-Output ("{0,-10} {1,-12} {2}" -f ($(if ($ok) { "PASS" } else { "FAIL" })), $gate, $note)
}

function Run-Gate([string]$name, [scriptblock]$body) {
    $out = Join-Path $OutDir ($name + ".log")
    & $body *> $out
    return $LASTEXITCODE
}

$head = (git rev-parse HEAD)
$dirty = (git status --porcelain | Measure-Object -Line).Lines
Write-Output "rc gates at $head (dirty entries: $dirty) -> $OutDir"

# 1 · check — exit 0 and not one warning line
$code = Run-Gate "check" { cargo check --workspace --all-targets }
$warns = (Select-String -Path (Join-Path $OutDir "check.log") -Pattern "^warning:" -CaseSensitive:$false | Measure-Object).Count
Record "check" (($code -eq 0) -and ($warns -eq 0)) "exit $code, $warns warning lines"

# 2 · tests — every target green; counts are printed from the log, not trusted
$code = Run-Gate "test" { cargo test --workspace --all-targets }
$testlog = Get-Content (Join-Path $OutDir "test.log")
$p = 0; $f = 0; $i = 0
foreach ($line in $testlog) {
    if ($line -match "^test result: ok\. (\d+) passed") { $p += [int]$Matches[1] }
    if ($line -match "^test result: FAILED") { $f += 1 }
    if ($line -match "^test result: ok.*; (\d+) ignored") { $i += [int]$Matches[1] }
}
Record "test" (($code -eq 0) -and ($f -eq 0)) "$p passed, $f failed targets, $i ignored (per-target: test.log)"

# 3 · release — exit 0 and zero warning lines
$code = Run-Gate "release" { cargo build --workspace --release }
$warns = (Select-String -Path (Join-Path $OutDir "release.log") -Pattern "^warning:" -CaseSensitive:$false | Measure-Object).Count
$exe = Get-Item "target/release/quire.exe" -ErrorAction SilentlyContinue
Record "release" (($code -eq 0) -and ($warns -eq 0)) "exit $code, $warns warning lines, exe $($exe.Length) B"

# 4 · sweep — every declared scene renders; the changed list is printed for
# the human, who judges it against diffbbox (a green run here is "no
# RENDER-FAIL", not "no pixel moved")
if ($Baseline -ne "") {
    $code = Run-Gate "sweep" { & benchmarks/scripts/sweep.ps1 -OutDir (Join-Path $OutDir "sweep") -Baseline $Baseline }
    $fails = (Select-String -Path (Join-Path $OutDir "sweep.log") -Pattern "RENDER-FAIL" | Measure-Object).Count
    $changed = (Select-String -Path (Join-Path $OutDir "sweep.log") -Pattern "^  changed" | Measure-Object).Count
    Record "sweep" (($code -eq 0) -and ($fails -eq 0)) "exit $code, $fails RENDER-FAIL (diff vs $Baseline in sweep.log)"
}
else {
    Record "sweep" $false "SKIPPED: no -Baseline given"
}

# 5 · bench audit — every stored summary recomputes from its raw rows
$code = Run-Gate "audit" { & benchmarks/scripts/audit_results.ps1 }
$aok = (Select-String -Path (Join-Path $OutDir "audit.log") -Pattern "audit ok" | Measure-Object).Count
Record "audit" (($code -eq 0) -and ($aok -gt 0)) "exit $code, 'audit ok' seen: $aok"

if (-not $SkipInstallers) {
    # 6 · installer — its own four groups; needs Inno Setup (Slice 1 removed
    # its deletes, so it is re-runnable)
    $code = Run-Gate "installer" { & install/verify-installer.ps1 }
    Record "installer" ($code -eq 0) "exit $code (4 groups; installer.log)"

    # 7 · portable — 24 checks; must never touch the real %APPDATA%\Quire
    $code = Run-Gate "portable" { & install/verify-portable.ps1 }
    $ok24 = (Select-String -Path (Join-Path $OutDir "portable.log") -Pattern "untouched" | Measure-Object).Count
    Record "portable" (($code -eq 0) -and ($ok24 -gt 0)) "exit $code, appdata-untouched line: $ok24 (24 checks in portable.log)"

    # 8 · dist — the zip exists when the run is over
    $code = Run-Gate "dist" { just dist }
    $zip = Get-Item "dist/quire-windows-x64.zip" -ErrorAction SilentlyContinue
    Record "dist" (($code -eq 0) -and ($null -ne $zip)) "exit $code, zip $(if ($zip) { $zip.Length } else { 'missing' }) B"
}
else {
    Record "installer" $false "SKIPPED (-SkipInstallers)"
    Record "portable" $false "SKIPPED (-SkipInstallers)"
    Record "dist" $false "SKIPPED (-SkipInstallers)"
}

$failed = @($rows | Where-Object { -not $_.ok })
Write-Output ""
Write-Output ("gates: {0} green, {1} not green (see {2})" -f ($rows.Count - $failed.Count), $failed.Count, $log)
$rows | Format-Table -AutoSize | Out-File -FilePath $log -Append
Get-Content (Join-Path $OutDir "gates.log") -ErrorAction SilentlyContinue | Out-Null
Write-Output "log dir: $OutDir"
