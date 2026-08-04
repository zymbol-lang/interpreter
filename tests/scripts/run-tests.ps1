<#
.SYNOPSIS
    The Zymbol correctness suites, on Windows, without bash.

.DESCRIPTION
    A native port of the four shell suites that judge the interpreter:

        cargo      cargo test --release
        vm         vm_compare.sh       - tree-walker and VM must print the same thing
        expected   expected_compare.sh - output against .expected golden files
        semantic   semantic_compare.sh - `zymbol check` diagnostics against goldens
        fmt        fmt_idempotency.sh  - formatting twice must change nothing

    Not a wrapper: it shells out to nothing but the interpreter itself. Requiring
    Git Bash to test a Windows build would mean the suite can only run where a
    POSIX toolchain was installed first, which is the assumption this whole hotfix
    branch exists to remove. The typed wildcards that the shell version delegates
    to python3 are implemented here with .NET regex, so there is no python
    dependency either.

    Two Windows details the shell suites never had to think about, both handled
    here rather than left to surprise the caller:

      * Encoding. The interpreter writes UTF-8; a PowerShell host reads a child's
        stdout in the console code page unless told otherwise. Without pinning it,
        every test with an accent or a pIqaD codepoint fails on the encoding alone.
      * Line endings. The .expected files are LF, committed from Linux; a process
        on Windows produces CRLF. Both sides are normalised to LF before comparing,
        so a real difference is never hidden behind an invisible one.

.PARAMETER Suite
    Which suites to run: all (default), or any of cargo, vm, expected, semantic, fmt.

.PARAMETER Filter
    Restrict `expected` and `semantic` to paths containing this substring.

.PARAMETER ZymbolBin
    Interpreter under test. Defaults to target\release\zymbol.exe in this repo.
    Point it at an installed binary to test a package instead of the build tree.

.PARAMETER TimeoutSec
    Per-file timeout. Default 10, matching the shell suites.

.PARAMETER Detail
    Print per-file results, not just failures and the summary.

.EXAMPLE
    .\tests\scripts\run-tests.ps1

.EXAMPLE
    .\tests\scripts\run-tests.ps1 -Suite vm -Detail

.EXAMPLE
    .\tests\scripts\run-tests.ps1 -Suite expected -Filter strings

.EXAMPLE
    .\tests\scripts\run-tests.ps1 -ZymbolBin "C:\Program Files\Zymbol-Lang\zymbol.exe" -Suite vm

.NOTES
    Exit code 0 when every selected suite passes, 1 otherwise, 2 when the
    interpreter is missing - a gate must never read "nothing ran" as "nothing
    failed".
#>

[CmdletBinding()]
param(
    # A plain string, split here rather than a [string[]] with [ValidateSet]:
    # `powershell -File script.ps1 -Suite vm,expected` hands the parameter one
    # string, so the validated-array form rejects the most natural way to call it.
    [string] $Suite = 'all',

    [string] $Filter = '',

    [string] $ZymbolBin = '',

    [int] $TimeoutSec = 10,

    [switch] $Detail
)

# Keep this file ASCII-only.
#
# Windows PowerShell 5.1 reads a script with no byte-order mark in the system ANSI
# code page, not UTF-8. A single em dash then arrives as the three CP1252 bytes
# a-euro-", and that last one is a curly quote - which PowerShell accepts as a
# string delimiter. The result is a parse error pointing at a line several
# statements away from the character that caused it. A BOM would also fix it, but
# only until the next editor drops it; staying inside ASCII cannot regress.

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# -- Paths --------------------------------------------------------------------

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot  = (Resolve-Path (Join-Path $ScriptDir '..\..')).Path
$TestsDir  = Join-Path $RepoRoot 'tests'

if (-not $ZymbolBin) {
    $ZymbolBin = Join-Path $RepoRoot 'target\release\zymbol.exe'
}

# -- Output -------------------------------------------------------------------

# The suite prints Zymbol's own output on failure, which is full of box drawing,
# accents and pIqaD. Without this the report itself is mojibake.
try { [Console]::OutputEncoding = [Text.Encoding]::UTF8 } catch { }

# UTF-8 for the child's stdin too, and specifically the *preamble-less* UTF-8.
#
# .NET Framework builds a process's StandardInput writer from Console.InputEncoding
# and sets AutoFlush on it, and setting AutoFlush flushes - which writes that
# encoding's preamble. So merely *reading* the StandardInput property was enough to
# put a UTF-8 BOM at the head of the pipe, before any test data. The interpreter
# then read U+FEFF as the first character of the first line: `<< #|n|` came back as
# the string "\u{feff}41" and every test with piped input failed on a conversion
# that had nothing wrong with it. Writing straight to BaseStream does not help,
# because the BOM is already in the pipe by then.
try { [Console]::InputEncoding = [Text.UTF8Encoding]::new($false) } catch { }

function Write-Head([string] $Text) {
    Write-Host ''
    Write-Host ('=' * 63) -ForegroundColor White
    Write-Host "  $Text" -ForegroundColor White
    Write-Host ('=' * 63) -ForegroundColor White
}

function Write-Pass([string] $Text) { if ($Detail) { Write-Host "  PASS  $Text" -ForegroundColor Green } }
function Write-Fail([string] $Text) { Write-Host "  FAIL  $Text" -ForegroundColor Red }
function Write-Skip([string] $Text) { if ($Detail) { Write-Host "  SKIP  $Text" -ForegroundColor Yellow } }

# -- Running the interpreter --------------------------------------------------

<#
    Quote one argument for a Windows command line.

    `ProcessStartInfo.ArgumentList`, which would make this unnecessary, only exists
    on .NET Core. Windows PowerShell 5.1 runs on .NET Framework, where the single
    `Arguments` string is all there is - so the quoting has to be done here, and it
    has to be right: every test path in this repo sits under "OneDrive - Abastible
    S.A", and an unquoted space would hand the interpreter three arguments that
    name nothing.

    The rule is the one CommandLineToArgvW documents: backslashes are literal
    except in the run immediately before a quote, where they are doubled.
#>
function ConvertTo-QuotedArg([string] $Argument) {
    if ($Argument -ne '' -and $Argument -notmatch '[ \t"]') { return $Argument }

    $sb = [Text.StringBuilder]::new()
    [void]$sb.Append('"')
    $backslashes = 0
    foreach ($ch in $Argument.ToCharArray()) {
        if ($ch -eq '\') {
            $backslashes++
            continue
        }
        if ($ch -eq '"') {
            [void]$sb.Append('\' * ($backslashes * 2 + 1))
            [void]$sb.Append('"')
        } else {
            [void]$sb.Append('\' * $backslashes)
            [void]$sb.Append($ch)
        }
        $backslashes = 0
    }
    # Trailing backslashes would otherwise escape the closing quote.
    [void]$sb.Append('\' * ($backslashes * 2))
    [void]$sb.Append('"')
    return $sb.ToString()
}

<#
    Run the interpreter and return stdout+stderr merged, or $null on timeout.

    Goes through `cmd /c ... 2>&1`, the way the shell suites go through `2>&1`,
    rather than reading two redirected pipes and concatenating them.

    That is not a detail. Two pipes cannot reproduce the *order* the streams
    interleave in: stdout is block-buffered when it is not a terminal, so a program
    that prints to stdout and then dies emits the error first and the buffered
    stdout afterwards. Concatenating stdout-then-stderr reverses that, and a golden
    recorded through `2>&1` will never match. `2>&1` hands both streams the same
    pipe and lets the operating system order them, which is what the goldens
    recorded.

    Letting cmd do the stdin redirection too removes a second problem: .NET builds
    a process's StandardInput writer from Console.InputEncoding and sets AutoFlush,
    which flushes - and so writes that encoding's preamble. Merely touching the
    property put a UTF-8 BOM at the head of the pipe, and the interpreter read
    U+FEFF as the first character of the first line.
#>
function Invoke-Zymbol {
    param(
        [string[]] $Arguments,
        [string]   $StdinFile = '',
        [string]   $WorkingDirectory = ''
    )

    $command = @($ZymbolBin) + $Arguments | ForEach-Object { ConvertTo-QuotedArg $_ }
    $command = $command -join ' '
    if ($StdinFile -and (Test-Path -LiteralPath $StdinFile)) {
        $command += ' < ' + (ConvertTo-QuotedArg $StdinFile)
    }
    $command += ' 2>&1'

    $psi = [Diagnostics.ProcessStartInfo]::new()
    $psi.FileName               = "$env:ComSpec"
    if ($WorkingDirectory) { $psi.WorkingDirectory = $WorkingDirectory }
    $psi.UseShellExecute        = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError  = $true
    $psi.StandardOutputEncoding = [Text.Encoding]::UTF8
    $psi.StandardErrorEncoding  = [Text.Encoding]::UTF8
    # `cmd /c "..."`: cmd strips the outermost pair of quotes and runs the rest, so
    # the inner quoting around each path survives intact.
    $psi.Arguments = '/c "' + $command + '"'

    $proc = [Diagnostics.Process]::Start($psi)

    $stdoutTask = $proc.StandardOutput.ReadToEndAsync()
    $stderrTask = $proc.StandardError.ReadToEndAsync()

    if (-not $proc.WaitForExit($TimeoutSec * 1000)) {
        # `Kill($true)`, which takes the process tree, is also .NET Core only.
        # taskkill does the same job and is always present.
        try { & taskkill.exe /F /T /PID $proc.Id 2>&1 | Out-Null } catch { }
        try { $proc.Kill() } catch { }
        return $null
    }

    # Give the readers their last chunk now that the pipes are closed.
    [void][Threading.Tasks.Task]::WaitAll(@($stdoutTask, $stderrTask), 5000)
    return ($stdoutTask.Result + $stderrTask.Result)
}

<#
    LF line endings, no trailing blank lines.

    `$(...)` in the shell strips trailing newlines, so the goldens were written
    without them; matching that here keeps a Windows run and a Linux run
    comparing the same text.
#>
function ConvertTo-Comparable([string] $Text) {
    if ($null -eq $Text) { return '' }
    return ($Text -replace "`r`n", "`n").TrimEnd("`n")
}

# -- Test file collection -----------------------------------------------------

function Get-ZyFiles {
    param(
        [string]   $Root,
        [string[]] $ExcludePathPatterns = @()
    )
    Get-ChildItem -LiteralPath $Root -Recurse -Filter '*.zy' -File |
        Where-Object {
            $rel = $_.FullName.Substring($TestsDir.Length).TrimStart('\')
            $keep = $true
            foreach ($pattern in $ExcludePathPatterns) {
                if ($rel -like $pattern) { $keep = $false; break }
            }
            $keep
        } |
        Sort-Object FullName
}

function Get-RelativePath([IO.FileInfo] $File) {
    return $File.FullName.Substring($TestsDir.Length).TrimStart('\')
}

# -- Golden matching ----------------------------------------------------------

# Same tokens the shell suite hands to python3, longest first so `***float***`
# is never matched as `****` followed by junk.
$script:WildcardTokens = [ordered] @{
    '***float***' = '-?[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?'
    '***time***'  = '[0-9]+(\.[0-9]+)?[mu\u00b5]?s'
    '***date***'  = '[0-9]{4}-[0-9]{2}-[0-9]{2}'
    '***path***'  = '\S+'
    '***int***'   = '-?[0-9]+'
    '***num***'   = '-?[0-9]+(\.[0-9]+)?'
    '****'        = '.*'
}

function ConvertTo-GoldenRegex([string] $Line) {
    $alternation = ($script:WildcardTokens.Keys | ForEach-Object { [regex]::Escape($_) }) -join '|'
    $parts = [regex]::Split($Line, "($alternation)")
    $sb = [Text.StringBuilder]::new()
    foreach ($part in $parts) {
        if ($script:WildcardTokens.Contains($part)) {
            [void]$sb.Append($script:WildcardTokens[$part])
        } else {
            [void]$sb.Append([regex]::Escape($part))
        }
    }
    return '^' + $sb.ToString() + '$'
}

function Test-MatchesGolden {
    param([string] $Actual, [string] $Golden)

    if ($Golden -notlike '*`*`*`**') { return $Actual -ceq $Golden }

    $actualLines = $Actual -split "`n"
    $goldenLines = $Golden -split "`n"
    if ($actualLines.Count -ne $goldenLines.Count) { return $false }

    for ($i = 0; $i -lt $goldenLines.Count; $i++) {
        $g = $goldenLines[$i]
        if ($g -like '*`*`*`**') {
            if ($actualLines[$i] -cnotmatch (ConvertTo-GoldenRegex $g)) { return $false }
        } elseif ($actualLines[$i] -cne $g) {
            return $false
        }
    }
    return $true
}

# -- Output normalisation, mirroring the shell suites -------------------------

# vm_compare.sh: redact what changes between two runs of the same program, so a
# fresh trace id is not reported as a tree-walker/VM disagreement.
function Remove-VolatileFields([string] $Text) {
    $t = $Text
    $t = $t -replace '"X-Amzn-Trace-Id": "[^"]*"', '"X-Amzn-Trace-Id": "<REDACTED>"'
    $t = $t -replace 'Root=[A-Za-z0-9;:-]*', 'Root=<REDACTED>'
    $t = $t -replace '"id": "[0-9a-f-]*"', '"id": "<UUID>"'
    $t = $t -replace '"uuid": "[0-9a-f-]*"', '"uuid": "<UUID>"'
    $t = $t -replace 'date: [A-Za-z]*, [0-9]* [A-Za-z]* [0-9]* [0-9:]* GMT', 'date: <DATE>'
    return $t
}

# expected_compare.sh's strip_warnings: compiler warnings and their indented
# continuation lines are not program output. Blank lines go too - the goldens
# were generated through this same filter, so keeping them would fail every pair.
function Remove-Warnings([string] $Text) {
    $kept = foreach ($line in ($Text -split "`n")) {
        if ($line -match '^warning:') { continue }
        if ($line -match '^  -->')    { continue }
        if ($line -match '^   ')      { continue }
        if ($line -match '^  =')      { continue }
        if ($line -match '^\s*$')     { continue }
        $line
    }
    return ($kept -join "`n")
}

# `e is a PowerShell 6 escape; on 5.1 it is just the letter e, so the pattern has
# to carry the escape character itself.
$script:Esc = [char] 27

function Remove-AnsiCodes([string] $Text) {
    return ($Text -replace "$($script:Esc)\[[0-9;]*[a-zA-Z]", '')
}

<#
    Make the file paths a diagnostic prints comparable across machines.

    Diagnostics echo the path they were given, so the goldens hold whatever the
    run that produced them was pointed at. Two things then differ off the original
    machine: the separator (`tests\errors\x.zy` here, `tests/errors/x.zy` in the
    golden), and, for the eight goldens that captured an absolute path, the whole
    prefix. Both are noise about where the repo happens to live, never about
    whether the diagnostic is right.
#>
function ConvertTo-PortablePaths([string] $Text) {
    $lines = foreach ($line in ($Text -split "`n")) {
        # Only the `-->` location line, never any line that merely mentions a .zy.
        #
        # A diagnostic quotes the offending source, and that quoted line can contain
        # backslashes of its own: rewriting every line with `.zy` in it turned
        # `<\ find . -name "*.zy" \>` into `</ ... />` and reported a difference in
        # code the interpreter had printed perfectly.
        if ($line -match '-->') { $line -replace '\\', '/' } else { $line }
    }
    return ($lines -join "`n")
}

# -- Suites -------------------------------------------------------------------

function Invoke-CargoSuite {
    Write-Head 'cargo test --release'
    Push-Location $RepoRoot
    try {
        & cargo test --release --no-fail-fast
        $ok = $LASTEXITCODE -eq 0
    } finally {
        Pop-Location
    }
    if ($ok) { Write-Host '  cargo test: OK' -ForegroundColor Green }
    else     { Write-Host '  cargo test: FAILED' -ForegroundColor Red }
    return [pscustomobject] @{ Name = 'cargo'; Pass = [int] $ok; Fail = [int] (-not $ok); Skip = 0; Failures = @() }
}

function Invoke-VmSuite {
    $files = @(Get-ZyFiles -Root $TestsDir -ExcludePathPatterns @(
        'scripts\*', '*\scripts\*',
        'stress_v2\*', '*\stress_v2\*',
        '*matematicas\module.zy'
    ))

    Write-Head "Tree-walker vs VM parity - $($files.Count) files"
    $pass = 0; $fail = 0; $skip = 0; $failures = @()

    foreach ($file in $files) {
        $rel = Get-RelativePath $file

        # @vm-skip on the first line marks a tree-walker-only feature.
        $firstLine = Get-Content -LiteralPath $file.FullName -TotalCount 1 -ErrorAction SilentlyContinue
        if ($firstLine -and $firstLine -match '@vm-skip') {
            $skip++; Write-Skip "$rel [vm-skip]"; continue
        }

        $stdin = [IO.Path]::ChangeExtension($file.FullName, '.input')

        $treeOut = Invoke-Zymbol -Arguments @('run', $file.FullName) -StdinFile $stdin
        $vmOut   = Invoke-Zymbol -Arguments @('run', '--vm', $file.FullName) -StdinFile $stdin

        if ($null -eq $treeOut -or $null -eq $vmOut) {
            $skip++; Write-Skip "$rel [timeout ${TimeoutSec}s]"; continue
        }

        $tree = ConvertTo-Comparable (Remove-VolatileFields $treeOut)
        $vm   = ConvertTo-Comparable (Remove-VolatileFields $vmOut)

        if ($tree -ceq $vm) {
            $pass++; Write-Pass $rel
        } else {
            $fail++
            # `elseif` has to share a line with the brace before it: a newline there
            # ends the statement and orphans the branch.
            if ($vm -match '^(VM compile error|Compile error|Parse error|Lex error|error\[)') {
                $why = '[VM error]'
            } elseif ($tree -match '^(Runtime error|Parse error|Lex error|error\[)') {
                $why = '[Tree error]'
            } else {
                $why = '[output mismatch]'
            }
            Write-Fail "$rel  $why"
            $failures += [pscustomobject] @{ Name = $rel; Expected = $tree; Actual = $vm; ExpectedLabel = 'Tree-walker'; ActualLabel = 'VM' }
        }
    }

    return [pscustomobject] @{ Name = 'vm'; Pass = $pass; Fail = $fail; Skip = $skip; Failures = $failures }
}

function Invoke-GoldenSuite {
    param(
        [string]   $Name,
        [string]   $Title,
        [string]   $Root,
        [string[]] $ExcludePathPatterns,
        [string[]] $CliArgs,          # e.g. @('run') or @('check')
        [scriptblock] $Normalise
    )

    if (-not (Test-Path -LiteralPath $Root)) {
        Write-Head "$Title - directory not found, skipped"
        return [pscustomobject] @{ Name = $Name; Pass = 0; Fail = 0; Skip = 0; Failures = @() }
    }

    # @(...) because a pipeline that matches exactly one file returns that file,
    # not a one-element array, and `.Count` on it is an error under StrictMode -
    # so a narrow -Filter broke the run while a broad one worked.
    $files = @(Get-ZyFiles -Root $Root -ExcludePathPatterns $ExcludePathPatterns |
        Where-Object {
            (Test-Path -LiteralPath ([IO.Path]::ChangeExtension($_.FullName, '.expected'))) -and
            ((-not $Filter) -or ((Get-RelativePath $_) -like "*$Filter*"))
        })

    Write-Head "$Title - $($files.Count) pairs"
    $pass = 0; $fail = 0; $failures = @()

    foreach ($file in $files) {
        $rel      = Get-RelativePath $file
        $goldFile = [IO.Path]::ChangeExtension($file.FullName, '.expected')
        $stdin    = [IO.Path]::ChangeExtension($file.FullName, '.input')

        # Run from the repo root with a repo-relative path, because that is how the
        # goldens were produced: a diagnostic echoes the path it was handed, and
        # most .expected files hold `tests/errors/...`, not an absolute path.
        $repoRelative = $file.FullName.Substring($RepoRoot.Length).TrimStart('\')
        $raw = Invoke-Zymbol -Arguments ($CliArgs + $repoRelative) -StdinFile $stdin -WorkingDirectory $RepoRoot
        if ($null -eq $raw) {
            $fail++
            Write-Fail "$rel  [timeout ${TimeoutSec}s]"
            $failures += [pscustomobject] @{ Name = $rel; Expected = '(golden)'; Actual = "timed out after ${TimeoutSec}s"; ExpectedLabel = 'Expected'; ActualLabel = 'Got' }
            continue
        }

        $actual = ConvertTo-Comparable (ConvertTo-PortablePaths (& $Normalise $raw))
        $golden = ConvertTo-Comparable ([IO.File]::ReadAllText($goldFile))

        if (Test-MatchesGolden -Actual $actual -Golden $golden) {
            $pass++; Write-Pass $rel
        } else {
            $fail++; Write-Fail $rel
            $failures += [pscustomobject] @{ Name = $rel; Expected = $golden; Actual = $actual; ExpectedLabel = 'Expected'; ActualLabel = 'Got' }
        }
    }

    return [pscustomobject] @{ Name = $Name; Pass = $pass; Fail = $fail; Skip = 0; Failures = $failures }
}

function Invoke-FmtSuite {
    $files = @(Get-ZyFiles -Root $TestsDir -ExcludePathPatterns @() |
        Where-Object { $_.Name -notlike 'bench_*' -and $_.Name -notlike 'stress*' -and $_.Name -notlike '_*' })

    Write-Head "Formatter idempotency - $($files.Count) files"
    $pass = 0; $fail = 0; $skip = 0; $failures = @()

    $work = Join-Path ([IO.Path]::GetTempPath()) ("zymbol_fmt_" + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $work | Out-Null
    try {
        foreach ($file in $files) {
            $rel   = Get-RelativePath $file
            $once  = Join-Path $work 'once.zy'
            $twice = Join-Path $work 'twice.zy'

            Copy-Item -LiteralPath $file.FullName -Destination $once -Force
            if ($null -eq (Invoke-Zymbol -Arguments @('fmt', $once, '--write'))) {
                $skip++; Write-Skip "$rel [fmt timeout]"; continue
            }
            if (-not (Test-Path -LiteralPath $once)) {
                # The formatter refuses files that do not parse; not its job.
                $skip++; Write-Skip "$rel [unformattable]"; continue
            }

            Copy-Item -LiteralPath $once -Destination $twice -Force
            [void](Invoke-Zymbol -Arguments @('fmt', $twice, '--write'))

            $a = [IO.File]::ReadAllText($once)
            $b = [IO.File]::ReadAllText($twice)
            if ($a -ceq $b) {
                $pass++; Write-Pass $rel
            } else {
                $fail++; Write-Fail "$rel  [not idempotent]"
                $failures += [pscustomobject] @{ Name = $rel; Expected = $a; Actual = $b; ExpectedLabel = 'First pass'; ActualLabel = 'Second pass' }
            }
        }
    } finally {
        Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
    }

    return [pscustomobject] @{ Name = 'fmt'; Pass = $pass; Fail = $fail; Skip = $skip; Failures = $failures }
}

# -- Main ---------------------------------------------------------------------

if (-not (Test-Path -LiteralPath $ZymbolBin)) {
    Write-Host "run-tests.ps1: interpreter not found: $ZymbolBin" -ForegroundColor Red
    Write-Host "  build it with 'cargo build --release', or pass -ZymbolBin." -ForegroundColor Yellow
    exit 2
}

$known    = @('cargo', 'vm', 'expected', 'semantic', 'fmt')
$asked    = @($Suite -split '[,\s]+' | Where-Object { $_ })
$unknown  = @($asked | Where-Object { $_ -ne 'all' -and $known -notcontains $_ })
if ($unknown.Count -gt 0) {
    Write-Host "run-tests.ps1: unknown suite(s): $($unknown -join ', ')" -ForegroundColor Red
    Write-Host "  choose from: all, $($known -join ', ')" -ForegroundColor Yellow
    exit 2
}
$selected = if ($asked -contains 'all') { $known } else { $asked }

Write-Host ''
Write-Host "Zymbol test suites - Windows" -ForegroundColor White
Write-Host "  interpreter : $ZymbolBin"
Write-Host "  version     : $((& $ZymbolBin --version) -join ' ')"
Write-Host "  suites      : $($selected -join ', ')"
if ($Filter) { Write-Host "  filter      : $Filter" }

$results = @()

foreach ($name in $selected) {
    switch ($name) {
        'cargo' { $results += Invoke-CargoSuite }
        'vm'    { $results += Invoke-VmSuite }
        'fmt'   { $results += Invoke-FmtSuite }
        'expected' {
            $results += Invoke-GoldenSuite `
                -Name 'expected' -Title 'Expected-output goldens' `
                -Root $TestsDir `
                -ExcludePathPatterns @('scripts\*', '*\scripts\*', '*matematicas\module.zy', 'errors\semantic\*', '*\errors\semantic\*') `
                -CliArgs @('run') `
                -Normalise { param($t) Remove-Warnings $t }
        }
        'semantic' {
            $results += Invoke-GoldenSuite `
                -Name 'semantic' -Title 'Semantic diagnostics (zymbol check)' `
                -Root (Join-Path $TestsDir 'errors\semantic') `
                -ExcludePathPatterns @('scripts\*', '*\scripts\*') `
                -CliArgs @('check') `
                -Normalise { param($t) Remove-AnsiCodes $t }
        }
    }
}

# -- Failure detail -----------------------------------------------------------

$allFailures = @($results | ForEach-Object { $_.Failures })
if ($allFailures.Count -gt 0) {
    Write-Head 'FAILURE DETAILS'
    foreach ($f in $allFailures) {
        Write-Host ''
        Write-Host "-- $($f.Name) --" -ForegroundColor Cyan
        Write-Host "  $($f.ExpectedLabel):" -ForegroundColor White
        ($f.Expected -split "`n" | Select-Object -First 20) | ForEach-Object { Write-Host "    $_" }
        Write-Host "  $($f.ActualLabel):" -ForegroundColor White
        ($f.Actual   -split "`n" | Select-Object -First 20) | ForEach-Object { Write-Host "    $_" }
    }
}

# -- Summary ------------------------------------------------------------------

Write-Head 'SUMMARY'
$totalFail = 0
foreach ($r in $results) {
    $totalFail += $r.Fail
    $colour = if ($r.Fail -gt 0) { 'Red' } elseif ($r.Skip -gt 0) { 'Yellow' } else { 'Green' }
    Write-Host ("  {0,-10} pass {1,5}   fail {2,5}   skip {3,5}" -f $r.Name, $r.Pass, $r.Fail, $r.Skip) -ForegroundColor $colour
}
Write-Host ''

if ($totalFail -eq 0) {
    Write-Host 'All selected suites pass.' -ForegroundColor Green
    exit 0
} else {
    Write-Host "$totalFail failure(s)." -ForegroundColor Red
    exit 1
}
