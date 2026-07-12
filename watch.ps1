[CmdletBinding()]
param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $WatchArguments
)

& cargo xtask watch @WatchArguments
exit $LASTEXITCODE
