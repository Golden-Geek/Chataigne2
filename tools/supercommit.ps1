param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $MessageParts
)

$ErrorActionPreference = "Stop"
$repoRoot = git rev-parse --show-toplevel 2>$null
if (-not $repoRoot) {
    throw "This script must run inside a Git repository."
}
Set-Location $repoRoot.Trim()

$commitMessage = if ($MessageParts) { $MessageParts -join " " } else { Read-Host "Commit message" }
if ([string]::IsNullOrWhiteSpace($commitMessage)) {
    throw "Commit message cannot be empty."
}

git add -A
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
git diff --cached --quiet
if ($LASTEXITCODE -eq 0) {
    Write-Host "No staged changes, skipping commit."
}
else {
    git commit -m $commitMessage
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}

Write-Host "Supercommit complete."
