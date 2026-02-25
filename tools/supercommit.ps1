param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $MessageParts
)

$ErrorActionPreference = "Stop"

$repoRoot = git rev-parse --show-toplevel 2>$null
if (-not $repoRoot) {
    Write-Error "Error: this script must run inside a Git repository."
    exit 1
}

Set-Location $repoRoot.Trim()

if ($MessageParts -and $MessageParts.Count -gt 0) {
    $commitMessage = ($MessageParts -join " ")
}
else {
    $commitMessage = Read-Host "Commit message"
}

if ([string]::IsNullOrWhiteSpace($commitMessage)) {
    Write-Error "Error: commit message cannot be empty."
    exit 1
}

$submoduleEntries = @(git config --file .gitmodules --get-regexp path 2>$null)
if ($LASTEXITCODE -ne 0) {
    $submoduleEntries = @()
}

foreach ($entry in $submoduleEntries) {
    if ([string]::IsNullOrWhiteSpace($entry)) {
        continue
    }

    $parts = $entry -split "\s+", 2
    if ($parts.Count -lt 2) {
        continue
    }

    $submodulePath = $parts[1].Trim()
    if (-not (Test-Path -Path $submodulePath)) {
        Write-Host "Skipping missing submodule path: $submodulePath"
        continue
    }

    $gitPointerPath = Join-Path $submodulePath ".git"
    if (-not (Test-Path -Path $gitPointerPath)) {
        Write-Host "Skipping uninitialized submodule: $submodulePath"
        continue
    }

    Write-Host "Processing submodule: $submodulePath"

    $branchName = git -C $submodulePath symbolic-ref --quiet --short HEAD 2>$null
    if (-not $branchName) {
        Write-Error "Error: submodule '$submodulePath' is in detached HEAD. Checkout a branch there first."
        exit 1
    }

    git -C $submodulePath add -A
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    git -C $submodulePath diff --cached --quiet
    if ($LASTEXITCODE -eq 0) {
        Write-Host "No staged changes in $submodulePath, skipping commit."
    }
    else {
        git -C $submodulePath commit -m $commitMessage
        if ($LASTEXITCODE -ne 0) {
            exit $LASTEXITCODE
        }
    }
}

Write-Host "Processing main repository..."
git add -A
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

git diff --cached --quiet
if ($LASTEXITCODE -eq 0) {
    Write-Host "No staged changes in main repository, skipping commit."
}
else {
    git commit -m $commitMessage
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}

Write-Host "Supercommit complete."
