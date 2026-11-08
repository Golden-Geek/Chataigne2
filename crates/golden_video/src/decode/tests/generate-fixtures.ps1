# Test-only encoder executable; production decode links the pinned FFmpeg libraries.
param([string]$Encoder = 'ffmpeg')
$ErrorActionPreference = 'Stop'
$taskFixtures = Join-Path $PSScriptRoot 'fixtures'
New-Item -ItemType Directory -Path $taskFixtures -Force | Out-Null
foreach ($taskCase in @(
    @{ Name = 'h264.mp4'; Rate = '24'; Codec = 'libx264'; Options = @('-crf', '12', '-bf', '2', '-g', '12'); Pixel = 'yuv420p' },
    @{ Name = 'hevc.mp4'; Rate = '30000/1001'; Codec = 'libx265'; Options = @('-x265-params', 'lossless=1:bframes=2:log-level=error:pools=none:frame-threads=1'); Pixel = 'yuv420p' },
    @{ Name = 'prores.mov'; Rate = '25'; Codec = 'prores_ks'; Options = @('-profile:v', '3'); Pixel = 'yuv422p10le' }
)) {
    $taskOptions = $taskCase.Options
    & $Encoder -hide_banner -loglevel error -y -f lavfi -i "nullsrc=s=64x48:r=$($taskCase.Rate),geq=lum='32+24*N':cb=128:cr=128" -frames:v 6 -an -c:v $taskCase.Codec @taskOptions -pix_fmt $taskCase.Pixel -color_range tv -colorspace bt709 -color_primaries bt709 -color_trc bt709 (Join-Path $taskFixtures $taskCase.Name)
    if ($LASTEXITCODE -ne 0) { throw "Fixture encoding failed: $($taskCase.Name)" }
}
& $Encoder -hide_banner -loglevel error -y -i (Join-Path $taskFixtures 'h264.mp4') -map 0:v:0 -c copy -bsf:v h264_mp4toannexb -f h264 (Join-Path $taskFixtures 'untimed.h264')
if ($LASTEXITCODE -ne 0) { throw 'Untimed fixture extraction failed' }
