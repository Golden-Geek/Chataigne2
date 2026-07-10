export const GOLDEN_UI_BOUNDARY = "golden-ui" as const;

export type PanelKind = "workspace" | "inspector" | "dashboard";

export interface PanelRegistration {
  id: string;
  kind: PanelKind;
  title: string;
}

export class PanelRegistry {
  readonly #panels = new Map<string, PanelRegistration>();

  register(panel: PanelRegistration): void {
    if (this.#panels.has(panel.id)) {
      throw new Error(`panel already registered: ${panel.id}`);
    }
    this.#panels.set(panel.id, Object.freeze({ ...panel }));
  }

  get(id: string): PanelRegistration | undefined {
    return this.#panels.get(id);
  }

  list(): readonly PanelRegistration[] {
    return [...this.#panels.values()];
  }
}
