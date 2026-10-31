import type { ApplicationPage } from '../media/generated/ApplicationPage';
import type { MediaEdit } from '../media/generated/MediaEdit';
import type { MediaEditResult } from '../media/generated/MediaEditResult';
import { ReadQueue } from '../requests/read-queue.ts';
import type { Connection } from '../requests/connection';
export type Invoke<S> = (session: S, edit: MediaEdit) => Promise<MediaEditResult>;
export interface CatalogState {
	page: ApplicationPage | null;
	search: string;
	start: number;
	loading: boolean;
	error: string;
}
export const catalogState = (): CatalogState => ({
	page: null,
	search: '',
	start: 0,
	loading: false,
	error: ''
});
export class ApplicationCatalog<S extends Connection> {
	private reads = new ReadQueue();
	private disposed = false;
	private session: () => S | null;
	readonly state: CatalogState;
	private invoke: Invoke<S>;
	constructor(session: () => S | null, state: CatalogState, invoke: Invoke<S>) {
		this.session = session;
		this.state = state;
		this.invoke = invoke;
	}
	refresh() {
		const session = this.session();
		if (this.disposed) return;
		if (!session || session.status !== 'connected' || session.loading.visible) {
			this.reset();
			return;
		}
		if (session.hasActiveEditSession) return;
		const generation = session.snapshotGeneration,
			history = session.currentHistoryStateId;
		const query = {
			kind: 'InspectCatalog' as const,
			search: this.state.search,
			start: this.state.start,
			count: 64
		};
		const current = () =>
			!this.disposed &&
			this.session() === session &&
			session.status === 'connected' &&
			!session.loading.visible &&
			session.snapshotGeneration === generation &&
			session.currentHistoryStateId === history &&
			this.state.search === query.search &&
			this.state.start === query.start;
		this.state.loading = true;
		this.reads.request(
			() => this.invoke(session, { kind: 'Applications', edit: query }),
			(result) => {
				if (!current()) return;
				if (result.kind !== 'Applications') {
					this.state.page = null;
					this.state.error = 'Unexpected application catalog';
					return;
				}
				this.state.page = result.page;
				this.state.error = '';
			},
			(error) => {
				if (current()) {
					this.state.page = null;
					this.state.error = String(error);
				}
			},
			() => {
				this.state.loading = false;
			}
		);
	}
	reset() {
		this.reads.invalidate();
		this.state.page = null;
		this.state.loading = false;
		this.state.error = '';
		this.state.start = 0;
	}
	dispose() {
		this.disposed = true;
		this.reset();
	}
}
