import { render } from "svelte/server";
import { describe, expect, it, vi } from "vitest";
import AudioRoutingPatchBay from "../AudioRoutingPatchBay.svelte";
import {
  audioRoutingCurvePath,
  audioRoutingPreviewCurvePath,
  emptyAudioRoutingPatchSelection,
  isAudioRoutingActivationKey,
  selectAudioRoutingPatchEndpoint,
  type AudioRoutingPatchBinding,
  type AudioRoutingPatchConnection,
  type AudioRoutingPatchEndpoint,
} from "../routing";

const binding = (): AudioRoutingPatchBinding => ({
  connect: vi.fn(async () => true),
  disconnect: vi.fn(async () => true),
  renameEndpoint: vi.fn(async () => true),
});

describe("AudioRoutingPatchBay", () => {
  it("renders endpoint rows and authored curves without a Cartesian control grid", () => {
    const sources: AudioRoutingPatchEndpoint[] = Array.from(
      { length: 256 },
      (_, index) => ({
        id: `source-${index}`,
        label: `Source ${index + 1}`,
        editable: true,
      }),
    );
    const destinations: AudioRoutingPatchEndpoint[] = Array.from(
      { length: 256 },
      (_, index) => ({
        id: `destination-${index}`,
        label: `Destination ${index + 1}`,
      }),
    );
    const connections: AudioRoutingPatchConnection[] = Array.from(
      { length: 32 },
      (_, index) => ({
        id: `route-${index}`,
        sourceId: sources[index].id,
        destinationId: destinations[255 - index].id,
      }),
    );

    const body = render(AudioRoutingPatchBay, {
      props: { sources, destinations, connections, binding: binding() },
    }).body;

    expect(body.match(/class="endpoint /g)).toHaveLength(512);
    expect(body.match(/connection-visible/g)).toHaveLength(32);
    expect(body.match(/connection-hit/g)).toHaveLength(32);
    expect(body.match(/<(button|input)/g)?.length).toBeLessThan(800);
    expect(body).not.toContain("65536");
    expect(body).toContain('aria-label="Drag Source 1 from Sources"');
    expect(body).toContain('aria-label="Drag Destination 1 from Destinations"');
    expect(body).not.toContain("Select Source 1");
  });

  it("renders crossing and one-to-many routes as labelled removable curves", () => {
    const sources = [
      { id: "output-1", label: "Output 1", editable: true },
      { id: "output-2", label: "Output 2", editable: true },
      { id: "output-3", label: "Output 3", editable: true },
    ];
    const destinations = [
      { id: "device-1", label: "Input 1" },
      { id: "device-2", label: "Input 2" },
      { id: "device-3", label: "Input 3" },
    ];
    const connections = [
      { id: "a", sourceId: "output-1", destinationId: "device-3" },
      { id: "b", sourceId: "output-2", destinationId: "device-1" },
      { id: "c", sourceId: "output-2", destinationId: "device-2" },
    ];

    const body = render(AudioRoutingPatchBay, {
      props: {
        sources,
        destinations,
        connections,
        binding: binding(),
        sourceLabel: "Output Channels",
        destinationLabel: "Device Outputs",
      },
    }).body;

    expect(body).toContain("M 1 0.5 C 38 0.5, 62 2.5, 99 2.5");
    expect(body).toContain("Remove route from Output 2 to Input 1");
    expect(body).toContain('aria-label="Rename Output 1"');
  });

  it("completes keyboard endpoint selection without choosing route policy", () => {
    const selected = selectAudioRoutingPatchEndpoint(
      emptyAudioRoutingPatchSelection(),
      "source",
      "source-a",
    );
    expect(selected.connection).toBeNull();

    const completed = selectAudioRoutingPatchEndpoint(
      selected.selection,
      "destination",
      "destination-b",
    );
    expect(completed.connection).toEqual({
      sourceId: "source-a",
      destinationId: "destination-b",
    });
    expect(completed.selection).toEqual(emptyAudioRoutingPatchSelection());
    expect(isAudioRoutingActivationKey("Enter")).toBe(true);
    expect(isAudioRoutingActivationKey(" ")).toBe(true);
    expect(isAudioRoutingActivationKey("Escape")).toBe(false);
    expect(audioRoutingCurvePath(3, 5)).toBe(
      "M 1 3.5 C 38 3.5, 62 5.5, 99 5.5",
    );
    expect(audioRoutingPreviewCurvePath("source", 3, 72, 5.25)).toBe(
      "M 1 3.5 C 38 3.5, 62 5.25, 72 5.25",
    );
    expect(audioRoutingPreviewCurvePath("destination", 5, 28, 3.25)).toBe(
      "M 28 3.25 C 38 3.25, 62 5.5, 99 5.5",
    );
  });
});
