/** Live capability; each panel persists its own typed, document-scoped target identity. */
export interface PanelContentBinding {
	locked: boolean;
	canLock: boolean;
	toggle: () => void;
}

export class PanelContentBindings {
	private values = new Map<string, PanelContentBinding>();
	private listeners = new Map<string, Set<(value: PanelContentBinding | null) => void>>();

	set(id: string, value: PanelContentBinding | null): void {
		if (value) this.values.set(id, value);
		else this.values.delete(id);
		for (const listener of this.listeners.get(id) ?? []) listener(value);
	}

	subscribe(id: string, listener: (value: PanelContentBinding | null) => void): () => void {
		const listeners = this.listeners.get(id) ?? new Set();
		this.listeners.set(id, listeners);
		listeners.add(listener);
		listener(this.values.get(id) ?? null);
		return () => {
			listeners.delete(listener);
			if (!listeners.size) this.listeners.delete(id);
		};
	}
}
