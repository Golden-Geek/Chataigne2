function Get-PinnedVideoInput($Definition, [string]$Destination) {
    $taskMatches = {
        param($Path)
        (Test-Path -LiteralPath $Path -PathType Leaf) -and
            (Get-Item -LiteralPath $Path).Length -eq $Definition.bytes -and
            (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() -eq $Definition.sha256
    }
    if (& $taskMatches $Destination) { return $Destination }
    if (Test-Path -LiteralPath $Destination) {
        Move-Item -LiteralPath $Destination -Destination ($Destination + '.rejected-' + [guid]::NewGuid().ToString('N'))
    }
    $taskPartial = $Destination + '.partial-' + [guid]::NewGuid().ToString('N')
    for ($taskAttempt = 0; $taskAttempt -lt 3; $taskAttempt++) {
        try {
            Invoke-WebRequest -UseBasicParsing -Uri $Definition.url -OutFile $taskPartial
            break
        } catch {
            if (Test-Path -LiteralPath $taskPartial) { Remove-Item -LiteralPath $taskPartial -Force }
            if ($taskAttempt -eq 2) { throw }
            Start-Sleep -Seconds 1
        }
    }
    if (!(& $taskMatches $taskPartial)) { throw "Pinned source input mismatch; retained at $taskPartial" }
    Move-Item -LiteralPath $taskPartial -Destination $Destination
    return $Destination
}
