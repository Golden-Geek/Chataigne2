# Test-only encoder executable; production decode links the pinned FFmpeg libraries.
param([string]$Encoder = 'ffmpeg', [string[]]$Only = @())
$ErrorActionPreference = 'Stop'
$taskFixtures = Join-Path $PSScriptRoot 'fixtures'
New-Item -ItemType Directory -Path $taskFixtures -Force | Out-Null
foreach ($taskCase in @(
    @{ Name = 'h264.mp4'; Rate = '24'; Codec = 'libx264'; Options = @('-crf', '12', '-bf', '2', '-g', '12'); Pixel = 'yuv420p' },
    @{ Name = 'h264-keyframes.mp4'; Rate = '24'; Codec = 'libx264'; Options = @('-crf', '12', '-bf', '2', '-g', '4', '-keyint_min', '4', '-sc_threshold', '0'); Pixel = 'yuv420p'; Frames = 12; Luma = '32+16*N' },
    @{ Name = 'hevc.mp4'; Rate = '30000/1001'; Codec = 'libx265'; Options = @('-x265-params', 'lossless=1:bframes=2:log-level=error:pools=none:frame-threads=1'); Pixel = 'yuv420p' },
    @{ Name = 'prores.mov'; Rate = '25'; Codec = 'prores_ks'; Options = @('-profile:v', '3'); Pixel = 'yuv422p10le' }
)) {
    if ($Only.Count -and $Only -notcontains $taskCase.Name) { continue }
    $taskOptions = $taskCase.Options
    $taskFrames = if ($taskCase.ContainsKey('Frames')) { $taskCase.Frames } else { 6 }
    $taskLuma = if ($taskCase.ContainsKey('Luma')) { $taskCase.Luma } else { '32+24*N' }
    & $Encoder -hide_banner -loglevel error -y -f lavfi -i "nullsrc=s=64x48:r=$($taskCase.Rate),geq=lum='$taskLuma':cb=128:cr=128" -frames:v $taskFrames -an -c:v $taskCase.Codec @taskOptions -pix_fmt $taskCase.Pixel -color_range tv -colorspace bt709 -color_primaries bt709 -color_trc bt709 (Join-Path $taskFixtures $taskCase.Name)
    if ($LASTEXITCODE -ne 0) { throw "Fixture encoding failed: $($taskCase.Name)" }
}
if (!$Only.Count -or $Only -contains 'untimed.h264') {
    & $Encoder -hide_banner -loglevel error -y -i (Join-Path $taskFixtures 'h264.mp4') -map 0:v:0 -c copy -bsf:v h264_mp4toannexb -f h264 (Join-Path $taskFixtures 'untimed.h264')
    if ($LASTEXITCODE -ne 0) { throw 'Untimed fixture extraction failed' }
}
