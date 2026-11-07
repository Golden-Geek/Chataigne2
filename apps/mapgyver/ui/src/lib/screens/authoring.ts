import { ReadQueue } from '../requests/read-queue.ts';
import type { Connection } from '../requests/connection';
import type { ScreenEditorQuery } from './generated/ScreenEditorQuery';
import type { ScreenEditorPage } from './generated/ScreenEditorPage';
import type { ScreenEdit } from './generated/ScreenEdit';
import type { ScreenEditResult } from './generated/ScreenEditResult';
import type { SurfaceSnapshot } from './generated/SurfaceSnapshot';

interface EditorConnection extends Connection {
	readonly projectGeneration: number | null;
}
export const screenEditorState = () => ({
	page: null as ScreenEditorPage | null,
	pending: false,
	refreshing: false,
	resolving: false,
	interacting: false,
	error: '',
	readError: '',
	presentedHistory: undefined as unknown
});
export type ScreenEditorState = ReturnType<typeof screenEditorState>;

/** One bounded reader and gesture owner per panel; no manager or media catalog reads. */
export class ScreenEditorController<S extends EditorConnection> {
	private reads = new ReadQueue();
	private disposed = false;
	private operation = 0;
	private context: { session: S; project: number } | null = null;
	private query: ScreenEditorQuery | null = null;
	private queryKey = '';
	private session: () => S | null;
	private state: ScreenEditorState;
	private invoke: (session: S, edit: ScreenEdit) => Promise<ScreenEditResult>;
	constructor(
		session: () => S | null,
		state: ScreenEditorState,
		invoke: (session: S, edit: ScreenEdit) => Promise<ScreenEditResult>
	) {
		this.session = session;
		this.state = state;
		this.invoke = invoke;
	}
	get page() {
		const page = this.state.page;
		const session = this.session();
		return session &&
			this.context?.session === session &&
			session.projectGeneration !== null &&
			this.context.project === session.projectGeneration
			? page
			: null;
	}
	get screen() {
		const page = this.page,
			screen = page?.screen,
			selected = page?.surface;
		return screen &&
			selected &&
			!screen.surfaces.some((item) => item.reference.surface === selected.reference.surface)
			? { ...screen, surfaces: [...screen.surfaces, selected] }
			: (screen ?? null);
	}
	get surface() {
		return this.page?.surface ?? null;
	}
	get busy() {
		const session = this.session();
		return (
			this.disposed ||
			this.state.pending ||
			this.state.resolving ||
			!this.page?.screen ||
			session?.status !== 'connected' ||
			session.loading.visible ||
			(!this.state.interacting &&
				!session.hasActiveEditSession &&
				this.state.presentedHistory !== session.currentHistoryStateId)
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
	refresh(query = this.query, resolve = false) {
		if (this.disposed || !query) return;
		this.query = query;
		const session = this.session();
		if (
			this.context &&
			(this.context.session !== session || this.context.project !== session?.projectGeneration)
		) {
			this.operation++;
			this.state.pending = false;
			this.state.interacting = false;
			this.state.page = null;
			this.state.error = '';
			this.state.readError = '';
			this.context = null;
		}
		if (this.state.interacting || this.state.pending) return;
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
			project = session.projectGeneration;
		if (resolve) this.reads.invalidate();
		const key = JSON.stringify(query),
			history = session.currentHistoryStateId,
			editing = session.hasActiveEditSession === true,
			revision = editing ? session.editSessionEpoch : history;
		if (resolve || key !== this.queryKey) this.state.resolving = true;
		this.queryKey = key;
		this.state.refreshing = true;
		const current = () =>
			this.current(session, generation, project) &&
			this.queryKey === key &&
			(session.hasActiveEditSession === true) === editing &&
			(editing
				? session.editSessionEpoch === revision
				: session.currentHistoryStateId === history) &&
			!this.state.interacting;
		this.reads.refresh(
			[session, generation, project, key, editing, revision],
			current,
			() => this.invoke(session, { kind: 'InspectEditor', query }),
			(result) => {
				if (!current()) return;
				if (result.kind !== 'Editor') {
					this.state.readError = 'Unexpected Screen editor response';
					return;
				}
				this.context = { session, project };
				this.state.page = result.page;
				this.state.presentedHistory = history;
				this.state.resolving = false;
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
	async edit(edit: ScreenEdit): Promise<ScreenEditResult | undefined> {
		const session = this.session();
		if (
			!session ||
			this.busy ||
			this.state.interacting ||
			session.hasActiveEditSession ||
			session.projectGeneration === null
		)
			return;
		const generation = session.snapshotGeneration,
			project = session.projectGeneration,
			operation = ++this.operation;
		this.reads.invalidate();
		this.state.pending = true;
		this.state.error = '';
		try {
			const result = await this.invoke(session, edit);
			if (operation === this.operation && this.current(session, generation, project)) return result;
		} catch (cause) {
			if (operation === this.operation && this.current(session, generation, project))
				this.state.error = String(cause);
		} finally {
			if (operation === this.operation) {
				this.state.pending = false;
				if (this.current(session, generation, project)) this.refresh();
			}
		}
	}
	acceptSurface(snapshot: SurfaceSnapshot) {
		const page = this.page;
		if (!page?.screen || page.screen.id !== snapshot.reference.screen) return;
		if (page.surface?.reference.surface === snapshot.reference.surface) page.surface = snapshot;
		const index = page.screen.surfaces.findIndex(
			(item) => item.reference.surface === snapshot.reference.surface
		);
		if (index >= 0) page.screen.surfaces[index] = snapshot;
	}
	setInteracting(active: boolean) {
		this.state.interacting = active;
		if (active) {
			this.reads.invalidate();
			this.state.refreshing = false;
		}
	}
	dispose() {
		this.disposed = true;
		this.operation++;
		this.reads.invalidate();
		this.context = null;
	}
}
