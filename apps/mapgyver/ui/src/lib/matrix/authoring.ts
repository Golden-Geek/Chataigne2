import type { MediaEdit } from '../media/generated/MediaEdit';
import type { MediaEditResult } from '../media/generated/MediaEditResult';
import type { MatrixEdit } from '../media/generated/MatrixEdit';
import type { MatrixItemId } from '../media/generated/MatrixItemId';
import type { MatrixViewport } from '../media/generated/MatrixViewport';
import type { MatrixViewportSnapshot } from '../media/generated/MatrixViewportSnapshot';
import type { MatrixSelection } from '../media/generated/MatrixSelection';
import type { MatrixEditorQuery } from '../media/generated/MatrixEditorQuery';
import type { MatrixEditorPage } from '../media/generated/MatrixEditorPage';
import type { Connection } from '../requests/connection';
import { ReadQueue } from '../requests/read-queue.ts';
import { sameSelection } from './selection.ts';
import { sameViewport } from './viewport.ts';

interface EditorConnection extends Connection {
	readonly projectGeneration: number | null;
}
export interface MatrixState {
	media: string | null;
	page: MatrixEditorPage | null;
	window: MatrixViewportSnapshot | null;
	viewport: MatrixViewport;
	selected: MatrixSelection | null;
	node: string | null;
	pending: boolean;
	refreshing: boolean;
	error: string;
	readError: string;
	interacting: boolean;
	history: unknown;
}
export const matrixState = (): MatrixState => ({
	media: null,
	page: null,
	window: null,
	viewport: { row_start: 0, row_count: 32, axis_start: 0, axis_count: 16 },
	selected: null,
	node: null,
	pending: false,
	refreshing: false,
	error: '',
	readError: '',
	interacting: false,
	history: undefined
});
export class MatrixController<S extends EditorConnection> {
	private reads = new ReadQueue();
	private disposed = false;
	private mutation = 0;
	private context: { session: S; project: number } | null = null;
	private query: MatrixEditorQuery | null = null;
	private queryKey = '';
	private session: () => S | null;
	readonly state: MatrixState;
	private invoke: (session: S, edit: MediaEdit) => Promise<MediaEditResult>;
	constructor(
		session: () => S | null,
		state: MatrixState,
		invoke: (session: S, edit: MediaEdit) => Promise<MediaEditResult>
	) {
		this.session = session;
		this.state = state;
		this.invoke = invoke;
	}
	get window() {
		const window = this.state.window,
			session = this.session();
		return !this.disposed &&
			session &&
			this.context?.session === session &&
			session.projectGeneration !== null &&
			this.context.project === session.projectGeneration
			? window
			: null;
	}
	get busy() {
		const session = this.session();
		return (
			this.disposed ||
			this.state.pending ||
			!this.window ||
			session?.status !== 'connected' ||
			session.loading.visible ||
			this.state.history !== session.currentHistoryStateId
		);
	}
	private current(session: S, generation: number, project: number) {
		return (
			!this.disposed &&
			this.session() === session &&
			session.status === 'connected' &&
			!session.loading.visible &&
			session.snapshotGeneration === generation &&
			session.projectGeneration === project
		);
	}
	observe(query: MatrixEditorQuery) {
		this.query = query;
		this.refresh();
	}
	restore(viewport: MatrixViewport, selection: MatrixSelection | null, node: string | null) {
		if (
			sameViewport(viewport, this.state.viewport) &&
			sameSelection(selection, this.state.selected) &&
			node === this.state.node
		)
			return;
		this.reads.invalidate();
		this.state.viewport = { ...viewport };
		this.state.selected = selection;
		this.state.node = node;
	}
	select(item: MatrixItemId | null, column: string | null) {
		if (sameSelection(this.state.selected, { item, column })) return;
		this.state.selected = { item, column };
		this.refresh();
	}
	setViewport(viewport: MatrixViewport) {
		if (this.disposed || sameViewport(viewport, this.state.viewport)) return;
		this.state.viewport = viewport;
		this.refresh();
	}
	refresh() {
		if (this.disposed || !this.query) return;
		const session = this.session();
		if (
			this.context &&
			(this.context.session !== session || this.context.project !== session?.projectGeneration)
		) {
			this.mutation++;
			this.context = null;
			this.state.pending = false;
			this.state.window = null;
			this.state.error = '';
			this.state.readError = '';
		}
		if (this.state.pending || this.state.interacting || session?.hasActiveEditSession) return;
		this.reads.invalidate();
		if (
			!session ||
			session.status !== 'connected' ||
			session.loading.visible ||
			session.projectGeneration === null
		) {
			this.state.refreshing = false;
			return;
		}
		const query = {
			...this.query,
			viewport: { ...this.state.viewport },
			selection: this.state.selected,
			node: this.state.node
		};
		const key = JSON.stringify(query),
			generation = session.snapshotGeneration,
			project = session.projectGeneration,
			history = session.currentHistoryStateId;
		// Until a new projection arrives, explicit intents still address the displayed target.
		// Reads must not disable focused controls between click and double-click/menu input.
		this.queryKey = key;
		this.state.refreshing = true;
		const current = () =>
			this.current(session, generation, project) &&
			this.queryKey === key &&
			session.currentHistoryStateId === history;
		this.reads.request(
			current,
			() => this.invoke(session, { kind: 'InspectMatrixEditor', query }),
			(result) => {
				if (!current()) return;
				if (result.kind !== 'MatrixEditor') {
					this.state.readError = 'Unexpected Matrix editor reply';
					return;
				}
				const changed = this.state.media !== result.page.media;
				this.context = { session, project };
				this.state.media = result.page.media;
				this.state.page = result.page;
				this.state.window = result.page.snapshot;
				this.state.selected = result.page.selection;
				this.state.node = result.page.node;
				if (changed && result.page.snapshot)
					this.state.viewport = {
						...this.state.viewport,
						row_start: result.page.snapshot.row_start,
						axis_start: result.page.snapshot.axis_start
					};
				this.state.history = history;
				this.state.readError = '';
			},
			(error) => {
				if (current()) this.state.readError = String(error);
			},
			() => {
				if (current()) this.state.refreshing = false;
			}
		);
	}
	async edit(edit: MatrixEdit): Promise<MediaEditResult | undefined> {
		const session = this.session(),
			media = this.state.media;
		if (
			!session ||
			!media ||
			this.busy ||
			session.hasActiveEditSession ||
			session.projectGeneration === null
		)
			return;
		const generation = session.snapshotGeneration,
			project = session.projectGeneration,
			mutation = ++this.mutation;
		this.reads.invalidate();
		this.state.pending = true;
		this.state.error = '';
		try {
			const result = await this.invoke(session, { kind: 'Matrix', media, edit });
			if (
				mutation === this.mutation &&
				this.current(session, generation, project) &&
				this.state.media === media
			)
				return result;
		} catch (error) {
			if (mutation === this.mutation && this.current(session, generation, project))
				this.state.error = String(error);
		} finally {
			if (mutation === this.mutation) {
				this.state.pending = false;
				if (this.current(session, generation, project)) this.refresh();
			}
		}
	}
	setInteracting(active: boolean) {
		this.state.interacting = active;
		if (active) this.reads.invalidate();
	}
	acceptLayer(result: Extract<MediaEditResult, { kind: 'MatrixLayerGeometryChanged' }>) {
		const window = this.window;
		if (!window || window.media !== result.media) return;
		for (const row of window.rows) {
			if (row.item.id.kind === 'Layer' && row.item.id.id === result.layer)
				row.item.layer = result.settings;
		}
		const selected = window.selected?.row;
		if (selected?.item.id.kind === 'Layer' && selected.item.id.id === result.layer)
			selected.item.layer = result.settings;
	}
	dispose() {
		this.disposed = true;
		this.mutation++;
		this.reads.invalidate();
		this.context = null;
	}
}
