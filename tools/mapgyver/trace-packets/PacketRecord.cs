using Microsoft.Windows.EventTracing;
using Microsoft.Windows.EventTracing.Graphics;

internal sealed record TimestampRecord(bool has_value, bool is_partial, long? qpc)
{
    public static TimestampRecord From(TraceTimestamp value) =>
        new(value.HasValue, value.IsPartial, !value.IsPartial && value.HasValue ? value.Value.RawValue : null);
}

internal sealed record DurationRecord(bool has_value, bool is_partial, long? nanoseconds)
{
    public static DurationRecord From(TraceDuration value) =>
        new(value.HasValue, value.IsPartial, !value.IsPartial && value.HasValue ? value.Nanoseconds : null);
}

internal sealed record PacketRecord(
    int pid, string role, int adapter_id, int node_id, int node_engine_id, string type,
    TimestampRecord software_submit, TimestampRecord hardware_submit,
    TimestampRecord execution_start, TimestampRecord execution_complete,
    DurationRecord software_queue, DurationRecord hardware_queue, DurationRecord execution)
{
    public static PacketRecord From(IDmaPacket packet, string role) => new(
        packet.SubmittingProcess.Id, role, packet.AdapterId, packet.NodeId, packet.NodeEngineId,
        packet.Type.ToString(), TimestampRecord.From(packet.SoftwareSubmitTime),
        TimestampRecord.From(packet.HardwareSubmitTime), TimestampRecord.From(packet.ExecutionStartTime),
        TimestampRecord.From(packet.ExecutionCompleteTime), DurationRecord.From(packet.SoftwareQueueDuration),
        DurationRecord.From(packet.HardwareQueueDuration), DurationRecord.From(packet.ExecutionDuration));
}
