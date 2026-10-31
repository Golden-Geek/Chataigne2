import type { Point } from './generated/Point';
import type { SurfaceReference } from './generated/SurfaceReference';
import type { SurfaceSnapshot } from './generated/SurfaceSnapshot';
import type { SurfaceGeometryEdit } from './generated/SurfaceGeometryEdit';

type Handle<T> = T extends { point: Point } ? Omit<T, 'point'> : never;
export type SurfaceHandle = Handle<SurfaceGeometryEdit>;

interface Host {
	readonly status: string;
	readonly snapshotGeneration: number;
	readonly loading: { readonly visible: boolean };
}
export interface EditGroup {
	readonly active: boolean;
	begin(): Promise<void>;
	end(): Promise<void>;
}
export interface SurfaceGestureState {
	active: boolean;
	closing: boolean;
	error: string;
}
interface Client<S> {
	group(host: S, handle: SurfaceHandle): EditGroup;
	move(host: S, reference: SurfaceReference, edit: SurfaceGeometryEdit): Promise<SurfaceSnapshot>;
	accepted(snapshot: SurfaceSnapshot): void;
	active(value: boolean): void;
	finished(): void;
}
export const createSurfaceGestureState = (): SurfaceGestureState => ({
	active: false,
	closing: false,
	error: ''
});

/** One in-flight sample and one latest point. Domain validity and history belong to the backend. */
export class SurfaceGesture<S extends Host> {
	private owner: {
		host: S;
		generation: number;
		reference: SurfaceReference;
		handle: SurfaceHandle;
		group: EditGroup;
		started: boolean;
	} | null = null;
	private latest: Point | null = null;
	private task: Promise<void> | null = null;
	private stopped = false;
	private currentHost: () => S | null;
	private state: SurfaceGestureState;
	private client: Client<S>;
	constructor(currentHost: () => S | null, state: SurfaceGestureState, client: Client<S>) {
		this.currentHost = currentHost;
		this.state = state;
		this.client = client;
	}
	start(reference: SurfaceReference, handle: SurfaceHandle): boolean {
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
	move(point: Point) {
		if (this.owner && !this.state.closing) {
			this.latest = point;
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
	private valid(owner: NonNullable<SurfaceGesture<S>['owner']>) {
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
				const point = this.latest;
				this.latest = null;
				try {
					const snapshot = await this.client.move(owner.host, owner.reference, {
						...owner.handle,
						point
					});
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
