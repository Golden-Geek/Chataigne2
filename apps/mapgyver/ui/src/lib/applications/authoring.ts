import type { ApplicationSnapshot } from '../media/generated/ApplicationSnapshot';
import { ReadQueue } from '../requests/read-queue.ts';
import type { MediaEdit } from '../media/generated/MediaEdit';
import type { MediaEditResult } from '../media/generated/MediaEditResult';
export type Invoke<S> = (session: S, edit: MediaEdit) => Promise<MediaEditResult>;
import type { Connection } from '../requests/connection';

export interface ApplicationState {
	selected: string | null;
	snapshot: ApplicationSnapshot | null;
	pending: boolean;
	loading: boolean;
	error: string;
	readError: string;
	missing: boolean;
}
export const applicationState = (): ApplicationState => ({
	selected: null,
	snapshot: null,
	pending: false,
	loading: false,
	error: '',
	readError: '',
	missing: false
});
export class ApplicationController<S extends Connection> {
	private reads = new ReadQueue();
	private disposed = false;
	private context: { session: S; generation: number } | null = null;
	private session: () => S | null;
	readonly state: ApplicationState;
	private invoke: Invoke<S>;
	constructor(session: () => S | null, state: ApplicationState, invoke: Invoke<S>) {
		this.session = session;
		this.state = state;
		this.invoke = invoke;
	}
	get busy() {
		const s = this.session();
		return (
			this.disposed ||
			this.state.pending ||
			this.state.loading ||
			s?.status !== 'connected' ||
			s.loading.visible ||
			s.hasActiveEditSession === true
		);
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
		if (
			this.context?.session !== session ||
			this.context.generation !== session.snapshotGeneration
		) {
			this.reset();
			this.context = { session, generation: session.snapshotGeneration };
		}
		if (session.hasActiveEditSession) return;
		this.refresh();
	}
	select(application: string | null) {
		this.reads.invalidate();
		this.state.selected = application;
		this.state.snapshot = null;
		this.state.loading = false;
		this.state.error = '';
		this.state.readError = '';
		this.state.missing = false;
		this.refresh();
	}
	refresh() {
		const session = this.session(),
			application = this.state.selected;
		if (
			this.disposed ||
			this.state.pending ||
			!session ||
			!application ||
			session.status !== 'connected' ||
			session.loading.visible ||
			session.hasActiveEditSession
		)
			return;
		const generation = session.snapshotGeneration,
			history = session.currentHistoryStateId;
		const current = () =>
			this.current(session, generation) &&
			this.state.selected === application &&
			session.currentHistoryStateId === history;
		this.state.loading = true;
		this.reads.request(
			current,
			() => this.invoke(session, { kind: 'Applications', edit: { kind: 'Inspect', application } }),
			(result) => {
				if (!current()) return;
				if (result.kind === 'ApplicationMissing' && result.application === application) {
					this.state.snapshot = null;
					this.state.missing = true;
					this.state.error = '';
					this.state.readError = '';
					return;
				}
				if (result.kind !== 'Application' || result.snapshot.id !== application) {
					this.state.snapshot = null;
					this.state.readError = 'Unexpected application snapshot';
					return;
				}
				this.state.snapshot = result.snapshot;
				this.state.missing = false;
				this.state.readError = '';
			},
			(error) => {
				if (current()) {
					this.state.snapshot = null;
					this.state.readError = String(error);
				}
			},
			() => {
				this.state.loading = false;
			}
		);
	}
	private reset() {
		this.reads.invalidate();
		Object.assign(this.state, applicationState());
		this.context = null;
	}
	dispose() {
		this.disposed = true;
		this.reset();
	}
}
