import type { ApplicationRuntimeSnapshot } from '../media/generated/ApplicationRuntimeSnapshot';
import type { Connection } from '../requests/connection';
import { ReadQueue } from '../requests/read-queue.ts';
import type { Invoke } from './catalog';

export interface RuntimeTransport {
	available(): boolean;
	subscribe(listener: (snapshot: ApplicationRuntimeSnapshot) => void): Promise<() => void>;
}
export const runtimeState = () => ({
	snapshot: null as ApplicationRuntimeSnapshot | null,
	loading: false,
	live: false,
	transportError: '',
	error: ''
});
export class ApplicationRuntimeReader<S extends Connection> {
	private session: () => S | null;
	private invoke: Invoke<S>;
	private transport: RuntimeTransport;
	readonly state: ReturnType<typeof runtimeState>;
	private reads = new ReadQueue();
	private context: { session: S; generation: number } | null = null;
	private subscription = 0;
	private unlisten: (() => void) | null = null;
	private project: string | null = null;
	private disposed = false;
	private retry: ReturnType<typeof setTimeout> | null = null;
	private bootstrapStarted = false;
	constructor(
		session: () => S | null,
		state: ReturnType<typeof runtimeState>,
		invoke: Invoke<S>,
		transport: RuntimeTransport
	) {
		this.session = session;
		this.state = state;
		this.invoke = invoke;
		this.transport = transport;
	}
	private current(session: S, generation: number) {
		return (
			!this.disposed &&
			this.session() === session &&
			session.status === 'connected' &&
			!session.loading.visible &&
			session.snapshotGeneration === generation
		);
	}
	observe() {
		if (this.disposed) return;
		const session = this.session();
		if (!session || session.status !== 'connected' || session.loading.visible) {
			this.reset();
			return;
		}
		if (this.context?.session === session && this.context.generation === session.snapshotGeneration)
			return;
		this.reset();
		this.context = { session, generation: session.snapshotGeneration };
		const generation = session.snapshotGeneration,
			token = ++this.subscription;
		if (!this.transport.available()) {
			this.refresh();
			return;
		}
		this.connect(session, generation, token);
	}
	private connect(session: S, generation: number, token: number) {
		void this.transport
			.subscribe((snapshot) => {
				if (token === this.subscription && this.current(session, generation)) this.accept(snapshot);
			})
			.then((unlisten) => {
				if (token !== this.subscription || !this.current(session, generation)) {
					unlisten();
					return;
				}
				this.unlisten = unlisten;
				this.state.live = true;
				this.state.transportError = '';
				this.bootstrapStarted = true;
				this.refresh(this.project === null);
			})
			.catch((error) => {
				if (token === this.subscription && this.current(session, generation)) {
					this.state.transportError = String(error);
					if (!this.bootstrapStarted) {
						this.bootstrapStarted = true;
						this.refresh();
					}
					this.retry = setTimeout(() => {
						this.retry = null;
						if (token === this.subscription && this.current(session, generation))
							this.connect(session, generation, token);
					}, 1000);
				}
			});
	}
	private accept(snapshot: ApplicationRuntimeSnapshot) {
		if (
			snapshot.project !== this.project ||
			(this.state.snapshot && snapshot.revision <= this.state.snapshot.revision)
		)
			return;
		this.state.snapshot = snapshot;
	}
	refresh(catchup = false) {
		const session = this.session();
		if (this.disposed || !session || session.status !== 'connected' || session.loading.visible)
			return;
		const generation = session.snapshotGeneration;
		this.state.loading = true;
		this.reads.request(
			() => this.invoke(session, { kind: 'Applications', edit: { kind: 'InspectRuntime' } }),
			(result) => {
				if (!this.current(session, generation)) return;
				if (result.kind !== 'ApplicationRuntime') {
					this.state.error = 'Unexpected application runtime snapshot';
					return;
				}
				if (this.project === null) this.project = result.snapshot.project;
				this.state.error = '';
				this.accept(result.snapshot);
				// Establish the project first, then close the subscribe/bootstrap race once. All
				// subsequent updates are revisioned events; the engine is never periodically polled.
				if (catchup) this.refresh();
			},
			(error) => {
				if (this.current(session, generation)) this.state.error = String(error);
			},
			() => {
				this.state.loading = false;
			}
		);
	}
	private reset() {
		++this.subscription;
		this.reads.invalidate();
		this.unlisten?.();
		this.unlisten = null;
		this.project = null;
		if (this.retry !== null) clearTimeout(this.retry);
		this.retry = null;
		this.bootstrapStarted = false;
		Object.assign(this.state, runtimeState());
		this.context = null;
	}
	dispose() {
		this.disposed = true;
		this.reset();
	}
}
