import type {
  AuthoringEvent,
  CatalogSnapshot,
  ControlResponse,
  PreviewChange,
  ScopeId,
  ServerMessage,
} from "./generated/protocol";
import { previewKey, RuntimeUiStore } from "./store";
import { decodeValueFrame } from "./value-frame";

export type FrameScheduler = (callback: (timestamp: number) => void) => number;

export interface FrameCommitMetrics {
  commits: number;
  stagedMessages: number;
  previewReplacements: number;
  binaryReplacements: number;
  maximumInputToPaintMs: number;
}

export class RuntimeFrameStager {
  readonly metrics: FrameCommitMetrics = {
    commits: 0,
    stagedMessages: 0,
    previewReplacements: 0,
    binaryReplacements: 0,
    maximumInputToPaintMs: 0,
  };

  private readonly authoring: AuthoringEvent[] = [];
  private readonly controls: ControlResponse[] = [];
  private readonly previews = new Map<string, PreviewChange>();
  private readonly resync = new Set<ScopeId>();
  private catalog?: CatalogSnapshot;
  private binary?: ArrayBuffer;
  private scheduled = false;
  private oldestInputAt?: number;

  constructor(
    private readonly store: RuntimeUiStore,
    private readonly schedule: FrameScheduler = (callback) => requestAnimationFrame(callback),
    private readonly maximumReliablePerFrame = 4_096,
  ) {}

  stageMessage(message: ServerMessage, receivedAt = performance.now()): void {
    this.noteInput(receivedAt);
    this.metrics.stagedMessages += 1;
    switch (message.plane) {
      case "control":
        this.pushReliable(this.controls, message.payload);
        break;
      case "authoring":
        this.pushReliable(this.authoring, message.payload);
        break;
      case "observation":
        if (message.payload.kind === "catalog") {
          this.catalog = message.payload.payload;
        } else if (message.payload.kind === "preview") {
          for (const change of message.payload.payload.changes) {
            const key = previewKey(change);
            if (this.previews.has(key)) this.metrics.previewReplacements += 1;
            this.previews.set(key, change);
          }
        } else {
          this.resync.add(message.payload.payload.scope);
        }
        break;
    }
    this.ensureScheduled();
  }

  stageBinary(buffer: ArrayBuffer, receivedAt = performance.now()): void {
    this.noteInput(receivedAt);
    if (this.binary) this.metrics.binaryReplacements += 1;
    this.binary = buffer;
    this.ensureScheduled();
  }

  takeControlResponses(): ControlResponse[] {
    return this.store.takeControlResponses();
  }

  private pushReliable<T>(target: T[], value: T): void {
    if (target.length === this.maximumReliablePerFrame) {
      throw new Error("reliable UI staging limit exceeded; transport backpressure failed");
    }
    target.push(value);
  }

  private noteInput(receivedAt: number): void {
    this.oldestInputAt = Math.min(this.oldestInputAt ?? receivedAt, receivedAt);
  }

  private ensureScheduled(): void {
    if (this.scheduled) return;
    this.scheduled = true;
    this.schedule((timestamp) => this.commit(timestamp));
  }

  private commit(timestamp: number): void {
    this.scheduled = false;
    this.store.applyControl(this.controls);
    if (this.catalog) this.store.applyCatalog(this.catalog);
    this.store.applyAuthoring(this.authoring);
    this.store.applyPreviews(this.previews.values());
    if (this.binary) this.store.applyValues(decodeValueFrame(this.binary));
    this.store.markResync(this.resync);
    this.authoring.length = 0;
    this.controls.length = 0;
    this.previews.clear();
    this.resync.clear();
    this.catalog = undefined;
    this.binary = undefined;
    this.store.frameRevision += 1;
    this.metrics.commits += 1;
    if (this.oldestInputAt !== undefined) {
      this.metrics.maximumInputToPaintMs = Math.max(
        this.metrics.maximumInputToPaintMs,
        timestamp - this.oldestInputAt,
      );
    }
    this.oldestInputAt = undefined;
  }
}
