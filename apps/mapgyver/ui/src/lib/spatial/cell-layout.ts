import type { Connection } from '../requests/connection';
import { ReadQueue } from '../requests/read-queue.ts';
import type { MatrixCellLayoutPage } from '../media/generated/MatrixCellLayoutPage';
import type { MatrixCellLayoutQuery } from '../media/generated/MatrixCellLayoutQuery';
import type { MediaEdit } from '../media/generated/MediaEdit';
import type { MediaEditResult } from '../media/generated/MediaEditResult';
interface Host extends Connection {
	readonly projectGeneration: number | null;
}
export const cellLayoutState = () => ({
	page: null as MatrixCellLayoutPage | null,
	error: '',
	refreshing: false,
	history: undefined as unknown,
	generation: 0
});
export class CellLayoutReader<S extends Host> {
	private reads = new ReadQueue();
	private disposed = false;
	private context: { session: S; project: number } | null = null;
	private key = '';
	private session: () => S | null;
	readonly state: ReturnType<typeof cellLayoutState>;
	private invoke: (session: S, edit: MediaEdit) => Promise<MediaEditResult>;
	constructor(
		session: () => S | null,
		state: ReturnType<typeof cellLayoutState>,
		invoke: (session: S, edit: MediaEdit) => Promise<MediaEditResult>
	) {
		this.session = session;
		this.state = state;
		this.invoke = invoke;
	}
	get page() {
		const page = this.state.page,
			session = this.session();
		return !this.disposed &&
			session &&
			this.context?.session === session &&
			this.context.project === session.projectGeneration
			? page
			: null;
	}
	get busy() {
		const session = this.session();
		return (
			!this.page ||
			session?.status !== 'connected' ||
			session.loading.visible ||
			session.hasActiveEditSession === true ||
			this.state.history !== session.currentHistoryStateId ||
			this.state.generation !== session.snapshotGeneration
		);
	}
	refresh(query: MatrixCellLayoutQuery) {
		if (this.disposed) return;
		const session = this.session();
		this.reads.invalidate();
		if (
			this.context &&
			(this.context.session !== session || this.context.project !== session?.projectGeneration)
		) {
			this.context = null;
			this.state.page = null;
			this.state.error = '';
		}
		if (
			!session ||
			session.status !== 'connected' ||
			session.loading.visible ||
			session.projectGeneration === null ||
			session.hasActiveEditSession
		) {
			this.state.refreshing = false;
			return;
		}
		const generation = session.snapshotGeneration,
			project = session.projectGeneration,
			history = session.currentHistoryStateId,
			key = JSON.stringify(query);
		this.key = key;
		this.state.refreshing = true;
		const current = () =>
			!this.disposed &&
			this.session() === session &&
			session.status === 'connected' &&
			!session.loading.visible &&
			session.snapshotGeneration === generation &&
			session.projectGeneration === project &&
			session.currentHistoryStateId === history &&
			this.key === key;
		this.reads.request(
			current,
			() => this.invoke(session, { kind: 'InspectMatrixCells', query }),
			(result) => {
				if (!current()) return;
				if (result.kind !== 'MatrixCellLayout') {
					this.state.error = 'Unexpected Cell layout response';
					return;
				}
				this.context = { session, project };
				this.state.page = result.page;
				this.state.error = '';
				this.state.history = history;
				this.state.generation = generation;
			},
			(error) => {
				if (current()) this.state.error = String(error);
			},
			() => {
				if (current()) this.state.refreshing = false;
			}
		);
	}
	dispose() {
		this.disposed = true;
		this.reads.invalidate();
		this.context = null;
	}
}
