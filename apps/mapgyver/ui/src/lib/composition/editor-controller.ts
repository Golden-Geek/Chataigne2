import type { CompositionEditorQuery } from '../media/generated/CompositionEditorQuery';
import type { CompositionEditorPage } from '../media/generated/CompositionEditorPage';
import type { CompositionEdit } from '../media/generated/CompositionEdit';
import type { InstanceSnapshot } from '../media/generated/InstanceSnapshot';
import type { MediaEdit } from '../media/generated/MediaEdit';
import type { MediaEditResult } from '../media/generated/MediaEditResult';
import type { Connection } from '../requests/connection';
import { ReadQueue } from '../requests/read-queue.ts';

export const createCompositionEditorState = () => ({
	page: null as CompositionEditorPage | null,
	pending: false,
	refreshing: false,
	resolving: false,
	interacting: false,
	error: '',
	readError: '',
	presentedHistory: undefined as unknown
});
export type CompositionEditorState = ReturnType<typeof createCompositionEditorState>;

/** One panel's bounded reads, immutable replies and mutations; domain decisions stay in Rust. */
export class CompositionEditorController<S extends Connection> {
	private reads = new ReadQueue();
	private operation = 0;
	private disposed = false;
	private context: { session: S; generation: number } | null = null;
	private query: CompositionEditorQuery | null = null;
	private queryKey = '';
	private getSession: () => S | null;
	private state: CompositionEditorState;
	private invoke: (session: S, edit: MediaEdit) => Promise<MediaEditResult>;
	constructor(
		getSession: () => S | null,
		state: CompositionEditorState,
		invoke: (session: S, edit: MediaEdit) => Promise<MediaEditResult>
	) {
		this.getSession = getSession;
		this.state = state;
		this.invoke = invoke;
	}
	get channel() {
		return this.state.page?.channel ?? null;
	}
	get instance() {
		return this.state.page?.selected ?? null;
	}
	get composition() {
		const composition = this.state.page?.composition,
			selected = this.instance;
		if (
			!composition ||
			!selected ||
			composition.instances.some((item) => item.reference.instance === selected.reference.instance)
		)
			return composition ?? null;
		return { ...composition, instances: [...composition.instances, selected] };
	}
	get busy() {
		const session = this.getSession();
		return (
			this.disposed ||
			this.state.pending ||
			this.state.resolving ||
			this.state.interacting ||
			session?.status !== 'connected' ||
			session.loading.visible ||
			(this.state.page !== null &&
				(this.state.presentedHistory !== session.currentHistoryStateId ||
					this.context?.session !== session ||
					this.context.generation !== session.snapshotGeneration))
		);
	}
	private current(session: S, generation: number) {
		return (
			!this.disposed &&
			this.getSession() === session &&
			session.status === 'connected' &&
			!session.loading.visible &&
			session.snapshotGeneration === generation
		);
	}
	refresh(query = this.query) {
		if (this.disposed || !query) return;
		this.query = query;
		const session = this.getSession();
		if (!session || session.status !== 'connected' || session.loading.visible) {
			this.reset();
			return;
		}
		const generation = session.snapshotGeneration;
		if (this.context?.session !== session || this.context.generation !== generation) this.reset();
		this.context = { session, generation };
		// Keep admitted geometry and loaded widgets usable while a grouped edit is held.
		if (this.state.pending || this.state.interacting || session.hasActiveEditSession) return;
		const key = JSON.stringify(query),
			history = session.currentHistoryStateId;
		if (key !== this.queryKey) {
			this.state.resolving = true;
			this.state.error = '';
		}
		this.queryKey = key;
		this.reads.invalidate();
		this.state.refreshing = true;
		const current = () =>
			this.current(session, generation) &&
			!session.hasActiveEditSession &&
			session.currentHistoryStateId === history &&
			this.queryKey === key;
		this.reads.request(
			current,
			() => this.invoke(session, { kind: 'InspectCompositionEditor', query }),
			(result) => {
				if (!current()) return;
				if (result.kind !== 'CompositionEditor') {
					this.state.readError = 'Unexpected Composition response';
					return;
				}
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
	edit(edit: CompositionEdit) {
		const reference = this.channel?.reference;
		if (reference) return this.mutate({ kind: 'Composition', reference, edit });
	}
	setInteracting(active: boolean) {
		this.state.interacting = active;
	}
	acceptInstance(snapshot: InstanceSnapshot) {
		const page = this.state.page,
			composition = page?.composition;
		if (
			!page ||
			!composition ||
			composition.reference.channel !== snapshot.reference.composition.channel ||
			composition.reference.media !== snapshot.reference.composition.media
		)
			return;
		if (page.selected?.reference.instance === snapshot.reference.instance) page.selected = snapshot;
		const index = composition.instances.findIndex(
			(item) => item.reference.instance === snapshot.reference.instance
		);
		if (index >= 0) composition.instances[index] = snapshot;
	}
	private async mutate(edit: MediaEdit): Promise<MediaEditResult | undefined> {
		const session = this.getSession();
		if (!session || this.busy || session.hasActiveEditSession) return;
		const generation = session.snapshotGeneration,
			operation = ++this.operation;
		this.reads.invalidate();
		this.state.pending = true;
		this.state.error = '';
		try {
			const result = await this.invoke(session, edit);
			if (operation !== this.operation || !this.current(session, generation)) return;
			if (result.kind === 'InstanceChanged') this.acceptInstance(result.snapshot);
			return result;
		} catch (cause) {
			if (operation === this.operation && this.current(session, generation))
				this.state.error = String(cause);
		} finally {
			if (operation === this.operation) {
				this.state.pending = false;
				if (this.current(session, generation)) this.refresh();
			}
		}
	}
	private reset() {
		this.reads.invalidate();
		++this.operation;
		this.context = null;
		this.queryKey = '';
		Object.assign(this.state, createCompositionEditorState());
	}
	dispose() {
		this.disposed = true;
		this.reset();
	}
}
