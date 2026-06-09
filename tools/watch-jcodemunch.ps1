$env:CODE_INDEX_PATH = Join-Path $env:USERPROFILE ".code-index"
$python = Join-Path $PSScriptRoot "..\.venv\Scripts\python.exe"
$watcher = Join-Path $PSScriptRoot "jcodemunch_workspace_watch.py"

if (-not (Test-Path -LiteralPath $python)) {
    throw "The project Python environment is not installed at $python"
}

if (-not (Test-Path -LiteralPath $watcher)) {
    throw "The jCodeMunch workspace watcher is not installed at $watcher"
}

& $python $watcher
exit $LASTEXITCODE
