import type { EditorReadHost as Connection } from 'golden_ui/store/editor-read-queue';
import {
	EditorReadQueue as ReadQueue,
	editorHistoryMatches
} from 'golden_ui/store/editor-read-queue';
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
	generation: 0,
	interacting: false
});
export class CellLayoutReader<S extends Host> {
	private reads = new ReadQueue();
	private disposed = false;
	private context: { session: S; project: number } | null = null;
	private key = '';
	private query: MatrixCellLayoutQuery | null = null;
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
			this.state.interacting ||
			!this.page ||
			session?.status !== 'connected' ||
			session.loading.visible ||
			session.hasActiveEditSession === true ||
			!editorHistoryMatches(session, this.state.history) ||
			this.state.generation !== session.snapshotGeneration
		);
	}
	refresh(query?: MatrixCellLayoutQuery, resolve = false) {
		if (this.disposed) return;
		if (query) this.query = query;
		if (!this.query) return;
		const request = this.query;
		const session = this.session();
		if (
			this.context &&
			(this.context.session !== session || this.context.project !== session?.projectGeneration)
		) {
			this.context = null;
			this.state.page = null;
			this.state.error = '';
		}
		if (this.state.interacting) return;
		if (resolve) this.reads.invalidate();
		if (
			!session ||
			session.status !== 'connected' ||
			session.loading.visible ||
			session.projectGeneration === null
		) {
			this.reads.invalidate();
			this.state.refreshing = false;
			return;
		}
		const generation = session.snapshotGeneration,
			project = session.projectGeneration,
			history = session.currentHistoryStateId,
			key = JSON.stringify(request);
		this.key = key;
		this.state.refreshing = true;
		const current = () =>
			!this.disposed &&
			this.session() === session &&
			session.status === 'connected' &&
			!session.loading.visible &&
			session.snapshotGeneration === generation &&
			session.projectGeneration === project &&
			this.key === key &&
			!this.state.interacting;
		this.reads.observe(
			session,
			key,
			current,
			() => this.invoke(session, { kind: 'InspectMatrixCells', query: request }),
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
	setInteracting(active: boolean) {
		this.state.interacting = active;
		if (active) this.reads.invalidate();
	}
	acceptCell(result: Extract<MediaEditResult, { kind: 'MatrixCellGeometryChanged' }>) {
		const page = this.page;
		if (!page || page.media !== result.media) return;
		for (const cell of page.cells)
			if (cell.view.cell.id === result.cell) cell.view.cell.definition.settings = result.settings;
		if (page.selected?.view.cell.id === result.cell)
			page.selected.view.cell.definition.settings = result.settings;
	}
	dispose() {
		this.disposed = true;
		this.reads.invalidate();
		this.context = null;
	}
}
