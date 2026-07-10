import type { ControlRequest, ObservationInterest, ServerMessage } from "./generated/protocol";
import type { RuntimeFrameStager } from "./frame-stager";

export interface RuntimeTransport {
  sendControl(request: ControlRequest): void;
  replaceInterest(interest: ObservationInterest): void;
  onMessage(handler: (message: ServerMessage) => void): () => void;
  onBinary(handler: (frame: ArrayBuffer) => void): () => void;
  close(): void;
}

export class RuntimeSession {
  private readonly unsubscribe: (() => void)[];

  constructor(
    readonly transport: RuntimeTransport,
    readonly frames: RuntimeFrameStager,
  ) {
    this.unsubscribe = [
      transport.onMessage((message) => frames.stageMessage(message)),
      transport.onBinary((frame) => frames.stageBinary(frame)),
    ];
  }

  close(): void {
    for (const unsubscribe of this.unsubscribe.splice(0)) unsubscribe();
    this.transport.close();
  }
}
