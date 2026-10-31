import type { ApplicationEdit } from '../media/generated/ApplicationEdit';
import type { ApplicationSnapshot } from '../media/generated/ApplicationSnapshot';
import { ReadQueue } from '../requests/read-queue.ts';
import { ApplicationCatalog, type CatalogState, type Invoke } from './catalog.ts';
import type { Connection } from '../requests/connection';

export interface ApplicationState {
	selected: string | null;
	snapshot: ApplicationSnapshot | null;
	pending: boolean;
	loading: boolean;
	error: string;
	readError: string;
}
export const applicationState = (): ApplicationState => ({
	selected: null,
	snapshot: null,
	pending: false,
	loading: false,
	error: '',
	readError: ''
});
export class ApplicationController<S extends Connection> {
	readonly catalog: ApplicationCatalog<S>;
	private reads = new ReadQueue();
	private disposed = false;
	private mutation = 0;
	private context: { session: S; generation: number } | null = null;
	private session: () => S | null;
	readonly state: ApplicationState;
	private invoke: Invoke<S>;
	constructor(
		session: () => S | null,
		state: ApplicationState,
		catalog: CatalogState,
		invoke: Invoke<S>
	) {
		this.session = session;
		this.state = state;
		this.invoke = invoke;
		this.catalog = new ApplicationCatalog(session, catalog, invoke);
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
		this.catalog.refresh();
		this.refresh();
	}
	select(application: string | null) {
		this.reads.invalidate();
		this.state.selected = application;
		this.state.snapshot = null;
		this.state.loading = false;
		this.state.error = '';
		this.state.readError = '';
		this.refresh();
	}
	refresh() {
		const session = this.session(),
			application = this.state.selected;
		if (
			this.disposed ||
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
			() => this.invoke(session, { kind: 'Applications', edit: { kind: 'Inspect', application } }),
			(result) => {
				if (!current()) return;
				if (result.kind !== 'Application' || result.snapshot.id !== application) {
					this.state.snapshot = null;
					this.state.readError = 'Unexpected application snapshot';
					return;
				}
				this.state.snapshot = result.snapshot;
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
	async edit(edit: ApplicationEdit) {
		const session = this.session(),
			selected = this.state.selected;
		if (!session || this.busy) return;
		const generation = session.snapshotGeneration,
			mutation = ++this.mutation;
		this.reads.invalidate();
		this.state.pending = true;
		this.state.error = '';
		try {
			const result = await this.invoke(session, { kind: 'Applications', edit });
			if (
				mutation !== this.mutation ||
				!this.current(session, generation) ||
				this.state.selected !== selected
			)
				return;
			if (result.kind === 'ApplicationCreated') this.select(result.application);
			else if (edit.kind === 'Remove' && edit.application === selected) this.select(null);
		} catch (error) {
			if (
				mutation === this.mutation &&
				this.current(session, generation) &&
				this.state.selected === selected
			)
				this.state.error = String(error);
		} finally {
			if (mutation === this.mutation) {
				this.state.pending = false;
				if (this.current(session, generation)) {
					this.catalog.refresh();
					this.refresh();
				}
			}
		}
	}
	private reset() {
		++this.mutation;
		this.reads.invalidate();
		this.catalog.reset();
		Object.assign(this.state, applicationState());
		this.context = null;
	}
	dispose() {
		this.disposed = true;
		this.reset();
		this.catalog.dispose();
	}
}
