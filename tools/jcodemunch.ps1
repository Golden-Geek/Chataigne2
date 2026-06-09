param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $JCodeMunchArgs
)

$env:CODE_INDEX_PATH = Join-Path $env:USERPROFILE ".code-index"
$executable = Join-Path $PSScriptRoot "..\.venv\Scripts\jcodemunch-mcp.exe"

if (-not (Test-Path -LiteralPath $executable)) {
    throw "jCodeMunch is not installed at $executable"
}

& $executable @JCodeMunchArgs
exit $LASTEXITCODE
