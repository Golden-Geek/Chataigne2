import type {
  AuthoringEvent,
  CatalogEntry,
  CatalogSnapshot,
  PreviewChange,
  ControlResponse,
  ScopeId,
} from "./generated/protocol";
import type { DecodedValueFrame } from "./value-frame";

export class RuntimeUiStore {
  readonly catalog = new Map<string, CatalogEntry>();
  readonly previews = new Map<string, PreviewChange>();
  readonly values = new Map<number, number>();
  readonly resyncScopes = new Set<ScopeId>();
  readonly controlResponses: ControlResponse[] = [];

  catalogRevision = 0;
  authoringRevision = 0;
  previewRevision = 0;
  valueRevision = 0;
  frameRevision = 0;
  generation = 0;
  valueSequence = 0;

  applyCatalog(snapshot: CatalogSnapshot): void {
    this.catalog.clear();
    for (const entry of snapshot.entries) this.catalog.set(entry.id, entry);
    this.catalogRevision = snapshot.revision;
  }

  applyAuthoring(events: readonly AuthoringEvent[]): void {
    if (events.length === 0) return;
    this.authoringRevision = events.at(-1)?.revision ?? this.authoringRevision;
  }

  applyControl(responses: readonly ControlResponse[]): void {
    this.controlResponses.push(...responses);
  }

  takeControlResponses(): ControlResponse[] {
    return this.controlResponses.splice(0);
  }

  applyPreviews(changes: Iterable<PreviewChange>): void {
    let changed = false;
    for (const change of changes) {
      this.previews.set(previewKey(change), change);
      changed = true;
    }
    if (changed) this.previewRevision += 1;
  }

  applyValues(frame: DecodedValueFrame): void {
    this.generation = frame.generation;
    this.valueSequence = frame.sequence;
    for (let index = 0; index < frame.slots.length; index += 1) {
      this.values.set(frame.slots[index]!, frame.values[index]!);
    }
    this.valueRevision += 1;
  }

  markResync(scopes: Iterable<ScopeId>): void {
    for (const scope of scopes) this.resyncScopes.add(scope);
  }
}

export function previewKey(change: PreviewChange): string {
  const { scope, entity, field } = change.key;
  return `${scope.length}:${scope}${entity.length}:${entity}${field.length}:${field}`;
}
