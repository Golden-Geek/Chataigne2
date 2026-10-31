export interface GestureHost {
	readonly status: string;
	readonly snapshotGeneration: number;
	readonly loading: { readonly visible: boolean };
}
export interface GestureEditGroup {
	readonly active: boolean;
	begin(): Promise<void>;
	end(): Promise<void>;
}
export interface LatestEditGestureState {
	active: boolean;
	closing: boolean;
	error: string;
}
export const createLatestEditGestureState = (): LatestEditGestureState => ({
	active: false,
	closing: false,
	error: ''
});
export interface LatestEditGestureClient<S, R, H, P, T> {
	group(host: S, handle: H): GestureEditGroup;
	move(host: S, reference: R, handle: H, sample: P): Promise<T>;
	accepted(snapshot: T): void;
	active(value: boolean): void;
	finished(): void;
}

/** Exclusive edit group with one in-flight request and one latest sample; no domain policy. */
export class LatestEditGesture<
	S extends GestureHost,
	R extends object,
	H extends object,
	P extends object,
	T
> {
	private owner: {
		host: S;
		generation: number;
		reference: R;
		handle: H;
		group: GestureEditGroup;
		started: boolean;
	} | null = null;
	private latest: P | null = null;
	private task: Promise<void> | null = null;
	private stopped = false;
	private currentHost: () => S | null;
	private state: LatestEditGestureState;
	private client: LatestEditGestureClient<S, R, H, P, T>;
	constructor(
		currentHost: () => S | null,
		state: LatestEditGestureState,
		client: LatestEditGestureClient<S, R, H, P, T>
	) {
		this.currentHost = currentHost;
		this.state = state;
		this.client = client;
	}
	start(reference: R, handle: H): boolean {
		const host = this.currentHost();
		if (this.stopped || this.owner || !host || host.status !== 'connected' || host.loading.visible)
			return false;
		this.owner = {
			host,
			generation: host.snapshotGeneration,
			reference: { ...reference },
			handle: { ...handle },
			group: this.client.group(host, handle),
			started: false
		};
		this.state.active = true;
		this.state.closing = false;
		this.state.error = '';
		this.client.active(true);
		this.run();
		return true;
	}
	move(sample: P) {
		if (this.owner && !this.state.closing) {
			this.latest = sample;
			this.run();
		}
	}
	finish() {
		if (this.owner) {
			this.state.closing = true;
			this.run();
		}
	}
	stop() {
		this.latest = null;
		this.finish();
	}
	dispose() {
		this.stopped = true;
		this.stop();
	}
	observe() {
		if (this.owner && (!this.valid(this.owner) || (this.owner.started && !this.owner.group.active)))
			this.stop();
	}
	private valid(owner: NonNullable<LatestEditGesture<S, R, H, P, T>['owner']>) {
		return (
			!this.stopped &&
			(!owner.started || owner.group.active) &&
			this.currentHost() === owner.host &&
			owner.host.status === 'connected' &&
			!owner.host.loading.visible &&
			owner.host.snapshotGeneration === owner.generation
		);
	}
	private run() {
		if (this.task || !this.owner) return;
		const task = this.pump();
		this.task = task;
		void task.finally(() => {
			this.task = null;
			if (this.owner && (this.latest || this.state.closing)) this.run();
		});
	}
	private async pump() {
		const owner = this.owner!;
		try {
			if (!owner.started) {
				await owner.group.begin();
				owner.started = true;
			}
			if (!owner.group.active) throw new Error('Another edit is active or the edit session ended');
			while (this.valid(owner) && this.latest) {
				const sample = this.latest;
				this.latest = null;
				try {
					const snapshot = await this.client.move(
						owner.host,
						owner.reference,
						owner.handle,
						sample
					);
					if (this.valid(owner)) {
						this.client.accepted(snapshot);
						this.state.error = '';
					}
				} catch (cause) {
					if (this.valid(owner)) this.state.error = String(cause);
				}
			}
			if (!this.valid(owner)) {
				this.latest = null;
				this.state.closing = true;
			}
		} catch (cause) {
			this.state.error = String(cause);
			this.latest = null;
			this.state.closing = true;
		} finally {
			if (this.state.closing) {
				try {
					await owner.group.end();
				} catch (cause) {
					this.state.error = String(cause);
				}
				this.owner = null;
				this.state.active = false;
				this.state.closing = false;
				this.client.active(false);
				this.client.finished();
			}
		}
	}
}
