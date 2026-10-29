# DirectX DMA packet observations

This app-owned, offline diagnostic uses Microsoft's pinned
[TraceProcessor](https://learn.microsoft.com/en-us/windows/apps/trace-processing/tutorial)
`UseDirectXData()` API. It exports only the renderer and three fixture PIDs from a native
qualification manifest. Golden's runtime, shaders, scheduling, DTOs and collector stay unchanged.
The tool neither starts a recording nor changes graphics settings.

From the repository root, with Windows x64 and the .NET 8 SDK/runtime available:

```powershell
dotnet restore tools/mapgyver/trace-packets/trace-packets.csproj --locked-mode
dotnet build tools/mapgyver/trace-packets/trace-packets.csproj -c Release --no-restore
dotnet tools/mapgyver/trace-packets/bin/Release/net8.0/trace-packets.dll trace.etl manifest.json new-packets.json
python -B tools/mapgyver/dma_packets.py new-packets.json manifest.json new-analysis.json
```

Inputs are hashed before/after processing. Outputs must be new files. Decoder assemblies,
bundled native-toolkit files, exporter assembly, source ETL and manifest hashes are retained.
The package graph is locked; build output stays ignored.

`has_value` alone is insufficient: TraceProcessor explicitly rejects reading a partial
timestamp's raw value. Both missing and partial API times remain null. The analyzer counts
every exported packet, keeps missing fields explicit and checks clock/frequency, native
identity, loss counters, boundary ordering and available duration agreement. Distributions
cover complete observed packets only; their denominator and missing count remain visible.
Loss or an empty role prevents complete timing coverage.

Even complete records would need exact context, source-image, batch and output joins before
qualifying MapGyver. Trace-local adapter IDs are not DXGI ordinals/LUIDs. This API does not
establish native-buffer occupancy or detect every missing packet.

The [2026-10-30 experiment](../../../docs/progress/mapgyver-m03-qualification-runner.md#dma-packet-decoder-boundary--2026-10-30)
returned 16,272 owned packets, all timings partial. Queue and GPU critical-path acceptance
remain unavailable. Its complete built-in WPR GPU capture produced 1.10 GiB in about ten
seconds, so that profile is not added to routine qualification. The system-wide ETL stays
local; published evidence contains only filtered records and owned native artifacts.
