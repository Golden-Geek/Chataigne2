import type {
  AudioBackendState,
  AudioDeviceDescriptor,
  AudioDeviceInspectorState,
  AudioDeviceTargetId,
  AudioDirection,
} from "./generated";

export interface AudioDeviceOption {
  readonly key: string;
  readonly label: string;
  readonly target: AudioDeviceTargetId;
  readonly missing: boolean;
}

export interface AudioDeviceOptionGroup {
  readonly backend: string;
  readonly backendLabel: string;
  readonly backendState: AudioBackendState;
  readonly backendDetail: string | null;
  readonly options: readonly AudioDeviceOption[];
}

export const audioDeviceTargetKey = (
  target: AudioDeviceTargetId | null,
): string => {
  if (!target) return "";
  return target.kind === "system_default"
    ? `default:${encodeURIComponent(target.backend)}`
    : `device:${encodeURIComponent(target.backend)}:${encodeURIComponent(target.device)}`;
};

const supportsDirection = (
  device: AudioDeviceDescriptor,
  direction: AudioDirection,
): boolean =>
  direction === "input"
    ? device.input_channels.length > 0
    : device.output_channels.length > 0;

export const audioDeviceOptionGroups = (
  state: AudioDeviceInspectorState,
  direction: AudioDirection,
): readonly AudioDeviceOptionGroup[] => {
  const backendIds = new Set(state.backends.map((backend) => backend.backend));
  for (const device of state.device_catalog)
    backendIds.add(device.target.backend);
  for (const device of state.devices) backendIds.add(device.target.backend);
  const catalogByKey = new Map(
    state.device_catalog.map((entry) => [
      audioDeviceTargetKey(entry.target),
      entry,
    ]),
  );
  const descriptorsByKey = new Map(
    state.devices.map((device) => [
      audioDeviceTargetKey(device.target),
      device,
    ]),
  );
  for (const device of state.devices) {
    const key = audioDeviceTargetKey(device.target);
    if (!catalogByKey.has(key))
      catalogByKey.set(key, { target: device.target, label: device.label });
  }

  const groups = [...backendIds].map((backendId) => {
    const status = state.backends.find(
      (backend) => backend.backend === backendId,
    );
    const options: AudioDeviceOption[] = [
      {
        key: audioDeviceTargetKey({
          kind: "system_default",
          backend: backendId,
        }),
        label: `System Default ${direction === "input" ? "Input" : "Output"}`,
        target: { kind: "system_default", backend: backendId },
        missing: false,
      },
    ];
    for (const [key, entry] of catalogByKey) {
      if (entry.target.backend !== backendId) continue;
      const descriptor = descriptorsByKey.get(key);
      if (descriptor && !supportsDirection(descriptor, direction)) continue;
      options.push({
        key,
        label: descriptor?.label ?? entry.label,
        target: entry.target,
        missing: false,
      });
    }
    return {
      backend: backendId,
      backendLabel: status?.label ?? backendId,
      backendState: status?.state ?? "unavailable",
      backendDetail: status?.detail ?? null,
      options,
    };
  });

  const stream = direction === "input" ? state.input : state.output;
  const selectedKey = audioDeviceTargetKey(stream.selected_target);
  if (
    stream.selected_target &&
    !groups.some((group) =>
      group.options.some((option) => option.key === selectedKey),
    )
  ) {
    groups.push({
      backend: stream.selected_target.backend,
      backendLabel: "Persisted selection",
      backendState: "unavailable",
      backendDetail:
        "The selected device is not present in the latest discovery result.",
      options: [
        {
          key: selectedKey,
          label: stream.selected_label ?? "Missing device",
          target: stream.selected_target,
          missing: true,
        },
      ],
    });
  }
  return groups;
};

export const findAudioDeviceTarget = (
  groups: readonly AudioDeviceOptionGroup[],
  key: string,
): AudioDeviceTargetId | null => {
  for (const group of groups) {
    const option = group.options.find((candidate) => candidate.key === key);
    if (option) return option.target;
  }
  return null;
};
