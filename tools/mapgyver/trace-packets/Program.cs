using System.Diagnostics;
using System.Security.Cryptography;
using System.Text.Json;
using Microsoft.Windows.EventTracing;

if (args.Length != 3)
{
    Console.Error.WriteLine("Usage: trace-packets <trace.etl> <native-manifest.json> <new-output.json>");
    return 2;
}

try
{
    var tracePath = Path.GetFullPath(args[0]);
    var manifestPath = Path.GetFullPath(args[1]);
    var outputPath = Path.GetFullPath(args[2]);
    if (File.Exists(outputPath)) throw new IOException("Prior evidence must not be overwritten.");
    var manifestBytes = File.ReadAllBytes(manifestPath);
    using var manifest = JsonDocument.Parse(manifestBytes);
    var root = manifest.RootElement;
    var roles = new Dictionary<int, string> { [root.GetProperty("pid").GetInt32()] = "renderer" };
    var fixtures = root.GetProperty("fixture_pids").EnumerateArray().ToArray();
    if (fixtures.Length != 3) throw new InvalidDataException("Expected two producers and one consumer.");
    foreach (var (fixture, role) in fixtures.Zip(new[] { "source-1", "source-2", "consumer" }))
    {
        var pid = fixture.GetInt32();
        if (pid <= 0 || !roles.TryAdd(pid, role)) throw new InvalidDataException("Invalid or duplicate owned PID.");
    }
    if (roles.Keys.Any(pid => pid <= 0)) throw new InvalidDataException("Invalid renderer PID.");

    // Hash before processing too: an edited/replaced file cannot silently become one artifact.
    var etlHash = HashFile(tracePath);
    var toolkitPath = Path.Combine(AppContext.BaseDirectory, "wpt");
    var toolkitFiles = Directory.EnumerateFiles(toolkitPath)
        .Where(path => Path.GetExtension(path) is ".dll" or ".exe")
        .Order().ToDictionary(path => Path.GetFileName(path)!, HashFile);
    using var trace = TraceProcessor.Create(tracePath, new TraceProcessorSettings { AllowLostEvents = true });
    var metadata = trace.UseMetadata();
    var pending = trace.UseDirectXData();
    trace.Process();
    var packets = pending.Result.DmaPackets;
    var owned = packets.Where(p => p.SubmittingProcess is not null && roles.ContainsKey(p.SubmittingProcess.Id))
        .Select(p => PacketRecord.From(p, roles[p.SubmittingProcess.Id])).ToArray();
    var modules = Process.GetCurrentProcess().Modules.Cast<ProcessModule>()
        .Where(m => m.ModuleName.StartsWith("Microsoft.Windows.EventTracing", StringComparison.OrdinalIgnoreCase)
            || m.ModuleName.Equals("perf_dx.dll", StringComparison.OrdinalIgnoreCase)
            || m.ModuleName.Equals("xperf.dll", StringComparison.OrdinalIgnoreCase))
        .Select(m => new { name = m.ModuleName, sha256 = HashFile(m.FileName) })
        .OrderBy(m => m.name).ToArray();
    if (etlHash != HashFile(tracePath) || !manifestBytes.SequenceEqual(File.ReadAllBytes(manifestPath)))
        throw new IOException("Trace or manifest changed during processing.");
    if (toolkitFiles.Any(pair => HashFile(Path.Combine(toolkitPath, pair.Key)) != pair.Value))
        throw new IOException("Decoder toolkit changed during processing.");
    var result = new
    {
        schema_version = 1,
        evidence_kind = "owned_process_directx_dma_packet_observations_not_qualification",
        run_id = root.GetProperty("run_id").GetString(),
        scenario = root.GetProperty("scenario").GetString(),
        manifest_sha256 = Convert.ToHexString(SHA256.HashData(manifestBytes)).ToLowerInvariant(),
        etl_sha256 = etlHash,
        etl_bytes = new FileInfo(tracePath).Length,
        clock_type = metadata.ClockType.ToString(),
        frequency = metadata.PerformanceCounterFrequency.Hertz,
        native_frequency = root.GetProperty("qpc_frequency").GetInt64(),
        lost_events = metadata.LostEventCount,
        lost_buffers = metadata.LostBufferCount,
        decoder = new
        {
            package_version = "1.12.10",
            runtime = Environment.Version.ToString(),
            modules,
            toolkit_files = toolkitFiles,
            exporter_sha256 = HashFile(typeof(PacketRecord).Assembly.Location)
        },
        roles = roles.Select(pair => new { pid = pair.Key, role = pair.Value }).ToArray(),
        owned_packets = owned,
        scope = "PID-filtered TraceProcessor DMA records; adapter/node IDs are trace-local, not DXGI ordinals or LUIDs",
        limitations = new[]
        {
            "Partial or missing API values remain null; packet count does not prove timestamp coverage.",
            "No completion token, source-image, D3D11 context or output association is supplied by this API.",
            "These packets do not establish native buffer occupancy, per-output queue time or full GPU critical path.",
            "The local ETL includes system-wide activity and is not included in published filtered evidence."
        }
    };
    using var output = new FileStream(outputPath, FileMode.CreateNew, FileAccess.Write, FileShare.None);
    JsonSerializer.Serialize(output, result, new JsonSerializerOptions { WriteIndented = true });
    Console.WriteLine($"Exported {owned.Length} owned DMA records; inspect partial fields before using timings.");
    return 0;
}
catch (Exception error)
{
    Console.Error.WriteLine($"DMA export failed: {error.Message}");
    return 1;
}

static string HashFile(string path)
{
    using var stream = File.OpenRead(path);
    return Convert.ToHexString(SHA256.HashData(stream)).ToLowerInvariant();
}
