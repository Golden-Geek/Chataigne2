export interface ExecutableSelectionSnapshot {
	path: string;
	browsing: boolean;
	error: string | null;
}

/** Form-local selection only; configuration commits remain with the authoring controller. */
export class ExecutableSelection {
	private active = true;
	private state: ExecutableSelectionSnapshot;
	private choose: () => Promise<string | null>;
	private enabled: () => boolean;
	private changed: (snapshot: ExecutableSelectionSnapshot) => void;

	constructor(
		path: string,
		choose: () => Promise<string | null>,
		enabled: () => boolean,
		changed: (snapshot: ExecutableSelectionSnapshot) => void
	) {
		this.state = { path, browsing: false, error: null };
		this.choose = choose;
		this.enabled = enabled;
		this.changed = changed;
	}

	private publish() {
		this.changed({ ...this.state });
	}

	setPath(path: string) {
		if (!this.active || this.state.browsing || !this.enabled()) return;
		this.state = { path, browsing: false, error: null };
		this.publish();
	}

	async browse() {
		if (!this.active || this.state.browsing || !this.enabled()) return;
		this.state = { ...this.state, browsing: true, error: null };
		this.publish();
		try {
			const selected = await this.choose();
			if (this.active && this.enabled() && selected !== null) {
				this.state.path = selected;
			}
		} catch (error) {
			if (this.active && this.enabled()) this.state.error = String(error);
		} finally {
			if (this.active) {
				this.state.browsing = false;
				this.publish();
			}
		}
	}

	dispose() {
		this.active = false;
	}
}
