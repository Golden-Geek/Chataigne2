export type AudioRoutingPatchSide = "source" | "destination";

export interface AudioRoutingPatchEndpoint {
  readonly id: string;
  readonly label: string;
  readonly editable?: boolean;
}

export interface AudioRoutingPatchConnection {
  readonly id: string;
  readonly sourceId: string;
  readonly destinationId: string;
}

export interface AudioRoutingPatchBinding {
  /**
   * Returns whether the intent was dispatched. The authoritative route state
   * remains the backend projection supplied through `connections`.
   */
  connect(sourceId: string, destinationId: string): Promise<boolean>;
  disconnect(connectionId: string): Promise<boolean>;
  renameEndpoint(
    side: AudioRoutingPatchSide,
    endpointId: string,
    label: string,
  ): Promise<boolean>;
}

export interface AudioRoutingPatchSelection {
  readonly sourceId: string | null;
  readonly destinationId: string | null;
}

export interface AudioRoutingPatchSelectionResult {
  readonly selection: AudioRoutingPatchSelection;
  readonly connection: {
    readonly sourceId: string;
    readonly destinationId: string;
  } | null;
}

export const emptyAudioRoutingPatchSelection =
  (): AudioRoutingPatchSelection => ({
    sourceId: null,
    destinationId: null,
  });

export const selectAudioRoutingPatchEndpoint = (
  selection: AudioRoutingPatchSelection,
  side: AudioRoutingPatchSide,
  endpointId: string,
): AudioRoutingPatchSelectionResult => {
  const next: AudioRoutingPatchSelection =
    side === "source"
      ? {
          sourceId: selection.sourceId === endpointId ? null : endpointId,
          destinationId: selection.destinationId,
        }
      : {
          sourceId: selection.sourceId,
          destinationId:
            selection.destinationId === endpointId ? null : endpointId,
        };

  if (next.sourceId === null || next.destinationId === null) {
    return { selection: next, connection: null };
  }
  return {
    selection: emptyAudioRoutingPatchSelection(),
    connection: {
      sourceId: next.sourceId,
      destinationId: next.destinationId,
    },
  };
};

export const audioRoutingCurvePath = (
  sourceIndex: number,
  destinationIndex: number,
): string => {
  const sourceY = sourceIndex + 0.5;
  const destinationY = destinationIndex + 0.5;
  return `M 1 ${sourceY} C 38 ${sourceY}, 62 ${destinationY}, 99 ${destinationY}`;
};

export const audioRoutingPreviewCurvePath = (
  side: AudioRoutingPatchSide,
  endpointIndex: number,
  pointerX: number,
  pointerY: number,
): string => {
  const endpointY = endpointIndex + 0.5;
  return side === "source"
    ? `M 1 ${endpointY} C 38 ${endpointY}, 62 ${pointerY}, ${pointerX} ${pointerY}`
    : `M ${pointerX} ${pointerY} C 38 ${pointerY}, 62 ${endpointY}, 99 ${endpointY}`;
};

export const isAudioRoutingActivationKey = (key: string): boolean =>
  key === "Enter" || key === " ";
