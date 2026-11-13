# Test-only encoder executable; production decode links the pinned FFmpeg libraries.
param([string]$Encoder = 'ffmpeg', [string[]]$Only = @())
$ErrorActionPreference = 'Stop'
$taskFixtures = Join-Path $PSScriptRoot 'fixtures'
New-Item -ItemType Directory -Path $taskFixtures -Force | Out-Null
foreach ($taskCase in @(
    @{ Name = 'h264.mp4'; Rate = '24'; Codec = 'libx264'; Options = @('-crf', '12', '-bf', '2', '-g', '12'); Pixel = 'yuv420p' },
    @{ Name = 'h264-keyframes.mp4'; Rate = '24'; Codec = 'libx264'; Options = @('-crf', '12', '-bf', '2', '-g', '4', '-keyint_min', '4', '-sc_threshold', '0'); Pixel = 'yuv420p'; Frames = 12; Luma = '32+16*N' },
    @{
        Name = 'h264-color.mp4'; Rate = '24'; Size = '1920x1080'; Codec = 'libx264'
        Options = @('-profile:v', 'high', '-level:v', '4.0', '-crf', '12', '-bf', '2', '-g', '12')
        Pixel = 'yuv420p'; Cb = 96; Cr = 160; Tagged = $true
    },
    @{
        Name = 'h264-bt2020.mp4'; Rate = '24'; Codec = 'libx264'
        Options = @('-crf', '12'); Pixel = 'yuv420p'
        Space = 'bt2020nc'; Transfer = 'bt2020-10'; Primaries = 'bt2020'
        Bitstream = 'h264_metadata=colour_primaries=9:transfer_characteristics=14:matrix_coefficients=9'
    },
    @{
        Name = 'h264-pq.mp4'; Rate = '24'; Codec = 'libx264'
        Options = @('-crf', '12'); Pixel = 'yuv420p'; Transfer = 'smpte2084'
        Bitstream = 'h264_metadata=colour_primaries=1:transfer_characteristics=16:matrix_coefficients=1'
    },
    @{ Name = 'hevc.mp4'; Rate = '30000/1001'; Codec = 'libx265'; Options = @('-x265-params', 'lossless=1:bframes=2:log-level=error:pools=none:frame-threads=1'); Pixel = 'yuv420p' },
    @{
        Name = 'hevc-main.mp4'; Rate = '30000/1001'; Size = '1920x1080'; Codec = 'libx265'
        Options = @('-x265-params', 'crf=12:bframes=2:level-idc=4:high-tier=0:log-level=error:pools=none:frame-threads=1')
        Pixel = 'yuv420p'
    },
    @{
        Name = 'hevc-main10.mp4'; Rate = '30000/1001'; Size = '1920x1080'; Codec = 'libx265'
        Options = @('-profile:v', 'main10', '-x265-params', 'crf=12:bframes=2:level-idc=4:high-tier=0:log-level=error:pools=none:frame-threads=1')
        Pixel = 'yuv420p10le'; Cb = 96; Cr = 160; Tagged = $true
    },
    @{ Name = 'prores.mov'; Rate = '25'; Codec = 'prores_ks'; Options = @('-profile:v', '3'); Pixel = 'yuv422p10le' },
    @{ Name = 'prores-odd.mov'; Rate = '25'; Size = '65x5'; Codec = 'prores_ks'; Options = @('-profile:v', '4'); Pixel = 'yuva444p10le' },
    @{
        Name = 'prores-alpha.mov'; Rate = '25'; Codec = 'prores_ks'
        Options = @('-profile:v', '4', '-alpha_bits', '16'); Pixel = 'yuva444p10le'; Tagged = $true
        Filter = "nullsrc=s=64x48:r=25,format=yuva444p10le,geq=lum='128+96*N':cb=384:cr=640:a='min(1023,256*N)'"
    }
)) {
    if ($Only.Count -and $Only -notcontains $taskCase.Name) { continue }
    $taskOptions = $taskCase.Options
    $taskFrames = if ($taskCase.ContainsKey('Frames')) { $taskCase.Frames } else { 6 }
    $taskSize = if ($taskCase.ContainsKey('Size')) { $taskCase.Size } else { '64x48' }
    $taskLuma = if ($taskCase.ContainsKey('Luma')) { $taskCase.Luma } else { '32+24*N' }
    $taskSpace = if ($taskCase.ContainsKey('Space')) { $taskCase.Space } else { 'bt709' }
    $taskTransfer = if ($taskCase.ContainsKey('Transfer')) { $taskCase.Transfer } else { 'bt709' }
    $taskPrimaries = if ($taskCase.ContainsKey('Primaries')) { $taskCase.Primaries } else { 'bt709' }
    $taskMetadata = if ($taskCase.ContainsKey('Bitstream')) { @('-bsf:v', $taskCase.Bitstream) } else { @() }
    $taskCb = if ($taskCase.ContainsKey('Cb')) { $taskCase.Cb } else { 128 }
    $taskCr = if ($taskCase.ContainsKey('Cr')) { $taskCase.Cr } else { 128 }
    $taskFilter = if ($taskCase.ContainsKey('Filter')) { $taskCase.Filter } else {
        "nullsrc=s=${taskSize}:r=$($taskCase.Rate),geq=lum='$taskLuma':cb=${taskCb}:cr=${taskCr}"
    }
    if ($taskCase.Tagged) {
        # Declare generated YUV before encoder negotiation; avoid an implicit 601-to-709 conversion.
        $taskFilter += ",setparams=range=limited:color_primaries=${taskPrimaries}:color_trc=${taskTransfer}:colorspace=${taskSpace}"
    }
    $taskArguments = @(
        '-hide_banner', '-loglevel', 'error', '-y', '-f', 'lavfi', '-i',
        $taskFilter,
        '-frames:v', $taskFrames, '-an', '-c:v', $taskCase.Codec
    ) + $taskOptions + @(
        '-pix_fmt', $taskCase.Pixel, '-color_range', 'tv', '-colorspace', $taskSpace,
        '-color_primaries', $taskPrimaries, '-color_trc', $taskTransfer
    ) + $taskMetadata + @((Join-Path $taskFixtures $taskCase.Name))
    & $Encoder @taskArguments
    if ($LASTEXITCODE -ne 0) { throw "Fixture encoding failed: $($taskCase.Name)" }
}
if (!$Only.Count -or $Only -contains 'untimed.h264') {
    & $Encoder -hide_banner -loglevel error -y -i (Join-Path $taskFixtures 'h264.mp4') -map 0:v:0 -c copy -bsf:v h264_mp4toannexb -f h264 (Join-Path $taskFixtures 'untimed.h264')
    if ($LASTEXITCODE -ne 0) { throw 'Untimed fixture extraction failed' }
}
if (!$Only.Count -or $Only -contains 'h264-growing.mp4') {
    $taskTempBase = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
    $taskParts = Join-Path $taskTempBase ('golden-video-growing-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $taskParts | Out-Null
    try {
        foreach ($taskPart in @(
            @{ Name = 'small.mp4'; Size = '64x48'; Frames = 144 },
            @{ Name = 'large.mp4'; Size = '3840x2160'; Frames = 2 }
        )) {
            & $Encoder -hide_banner -loglevel error -n -f lavfi -i "color=c=red:s=$($taskPart.Size):r=24" `
                -frames:v $taskPart.Frames -an -c:v libx264 -threads 4 -preset ultrafast -tune zerolatency `
                -x264-params 'keyint=1:repeat-headers=1' -pix_fmt yuv420p -color_range tv `
                -colorspace bt709 -color_primaries bt709 -color_trc bt709 (Join-Path $taskParts $taskPart.Name)
            if ($LASTEXITCODE -ne 0) { throw "Growing fixture segment failed: $($taskPart.Name)" }
        }
        [System.IO.File]::WriteAllText((Join-Path $taskParts 'concat.txt'), "file 'small.mp4'`nfile 'large.mp4'`n")
        & $Encoder -hide_banner -loglevel error -y -f concat -safe 0 -i (Join-Path $taskParts 'concat.txt') `
            -map 0:v:0 -c copy -movflags +faststart (Join-Path $taskFixtures 'h264-growing.mp4')
        if ($LASTEXITCODE -ne 0) { throw 'Growing fixture concatenation failed' }
    } finally {
        $taskResolvedParts = [System.IO.Path]::GetFullPath($taskParts)
        if ([System.IO.Path]::GetDirectoryName($taskResolvedParts).TrimEnd('\') -ne $taskTempBase.TrimEnd('\') `
            -or ![System.IO.Path]::GetFileName($taskResolvedParts).StartsWith('golden-video-growing-')) {
            throw 'Generated fixture cleanup target is outside its owned temporary directory'
        }
        Remove-Item -LiteralPath $taskResolvedParts -Recurse -Force
    }
}
