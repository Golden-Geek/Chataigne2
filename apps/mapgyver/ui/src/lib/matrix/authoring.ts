import type { MediaEdit } from '../media/generated/MediaEdit';
import type { MediaEditResult } from '../media/generated/MediaEditResult';
import type { MatrixEdit } from '../media/generated/MatrixEdit';
import type { MatrixItemId } from '../media/generated/MatrixItemId';
import type { MatrixViewport } from '../media/generated/MatrixViewport';
import type { MatrixViewportSnapshot } from '../media/generated/MatrixViewportSnapshot';
import { ChoiceReader, choiceState } from './choices.ts';
import type { Connection } from '../requests/connection';
import { ReadQueue } from '../requests/read-queue.ts';

export interface MatrixState {
	media: string | null;
	window: MatrixViewportSnapshot | null;
	viewport: MatrixViewport;
	selected: { item: MatrixItemId | null; column: string | null } | null;
	pending: boolean;
	refreshing: boolean;
	error: string;
	readError: string;
}
export const matrixState = (): MatrixState => ({
	media: null,
	window: null,
	viewport: { row_start: 0, row_count: 32, column_start: 0, column_count: 16 },
	selected: null,
	pending: false,
	refreshing: false,
	error: '',
	readError: ''
});
export class MatrixController<S extends Connection> {
	readonly matrices: ChoiceReader<S>;
	readonly sources: ChoiceReader<S>;
	private reads = new ReadQueue();
	private disposed = false;
	private mutation = 0;
	private context: { session: S; generation: number } | null = null;
	private session: () => S | null;
	readonly state: MatrixState;
	private invoke: (session: S, edit: MediaEdit) => Promise<MediaEditResult>;
	constructor(
		session: () => S | null,
		state: MatrixState,
		matrixChoices: ReturnType<typeof choiceState>,
		sourceChoices: ReturnType<typeof choiceState>,
		invoke: (session: S, edit: MediaEdit) => Promise<MediaEditResult>
	) {
		this.session = session;
		this.state = state;
		this.invoke = invoke;
		const connected = () => (this.disposed ? null : this.session());
		this.matrices = new ChoiceReader(connected, matrixChoices, 'MatrixFinal', invoke, () => {
			if (!this.state.media)
				this.selectMedia(matrixChoices.page?.choices[0]?.reference.media ?? null);
		});
		this.sources = new ChoiceReader(connected, sourceChoices, 'All', invoke);
	}
	get busy() {
		const session = this.session();
		return (
			this.disposed ||
			this.state.pending ||
			session?.status !== 'connected' ||
			session.loading.visible
		);
	}
	private current(session: S, generation: number, media: string | null) {
		return (
			!this.disposed &&
			this.session() === session &&
			session.status === 'connected' &&
			!session.loading.visible &&
			session.snapshotGeneration === generation &&
			this.state.media === media
		);
	}
	observe(catalog = true) {
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
		if (catalog) {
			this.matrices.refresh();
			this.sources.refresh();
		}
		if (!this.state.pending) this.refresh();
	}
	selectMedia(media: string | null) {
		if (this.state.pending || this.disposed) return;
		this.reads.invalidate();
		this.state.media = media;
		this.state.window = null;
		this.state.selected = null;
		this.state.viewport = { ...this.state.viewport, row_start: 0, column_start: 0 };
		this.refresh();
	}
	select(item: MatrixItemId | null, column: string | null) {
		this.state.selected = { item, column };
	}
	setViewport(viewport: MatrixViewport) {
		if (this.disposed) return;
		if (JSON.stringify(viewport) === JSON.stringify(this.state.viewport)) return;
		this.state.viewport = viewport;
		this.refresh();
	}
	refresh() {
		const session = this.session(),
			media = this.state.media;
		if (!session || !media || this.busy) return;
		const generation = session.snapshotGeneration,
			history = session.currentHistoryStateId,
			viewport = { ...this.state.viewport };
		this.state.refreshing = true;
		this.reads.request(
			() =>
				this.invoke(session, {
					kind: 'Matrix',
					media,
					edit: { kind: 'InspectViewport', viewport }
				}),
			(result) => {
				if (!this.current(session, generation, media) || history !== session.currentHistoryStateId)
					return;
				if (result.kind !== 'MatrixViewport' || result.snapshot.media !== media) {
					this.state.readError = 'Unexpected Matrix viewport';
					return;
				}
				this.state.window = result.snapshot;
				this.state.readError = '';
			},
			(error) => {
				if (this.current(session, generation, media) && history === session.currentHistoryStateId) {
					this.state.window = null;
					this.state.readError = String(error);
				}
			},
			() => {
				this.state.refreshing = false;
			}
		);
	}
	async edit(edit: MatrixEdit) {
		const media = this.state.media;
		if (media) await this.mutate({ kind: 'Matrix', media, edit });
	}
	async create() {
		await this.mutate({ kind: 'CreateMedia', media_kind: 'Matrix', label: null });
	}
	private async mutate(edit: MediaEdit) {
		const session = this.session(),
			media = this.state.media;
		if (!session || this.busy) return;
		const generation = session.snapshotGeneration,
			mutation = ++this.mutation;
		this.reads.invalidate();
		this.state.pending = true;
		this.state.error = '';
		try {
			const result = await this.invoke(session, edit);
			if (mutation !== this.mutation || !this.current(session, generation, media)) return;
			if (result.kind === 'Created') {
				this.state.media = result.reference.media;
				this.state.selected = null;
				this.state.window = null;
				this.state.viewport = { ...this.state.viewport, row_start: 0, column_start: 0 };
			}
		} catch (error) {
			if (mutation === this.mutation && this.current(session, generation, media))
				this.state.error = String(error);
		} finally {
			if (mutation === this.mutation) {
				this.state.pending = false;
				if (
					!this.disposed &&
					this.session() === session &&
					session.snapshotGeneration === generation
				) {
					this.refresh();
					if (
						edit.kind !== 'Matrix' ||
						(edit.edit.kind !== 'LaunchColumn' && edit.edit.kind !== 'ClearPlayback')
					) {
						this.matrices.refresh();
						this.sources.refresh();
					}
				}
			}
		}
	}
	private reset() {
		++this.mutation;
		this.reads.invalidate();
		this.matrices.reset();
		this.sources.reset();
		this.state.media = null;
		this.state.window = null;
		this.state.selected = null;
		this.state.pending = false;
		this.state.refreshing = false;
		this.state.error = '';
		this.state.readError = '';
		this.context = null;
	}
	dispose() {
		this.disposed = true;
		this.reset();
	}
}
