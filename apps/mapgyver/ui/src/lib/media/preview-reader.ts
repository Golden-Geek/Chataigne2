import type { MediaPreviewQuery } from './generated/MediaPreviewQuery';
import type { MediaPreviewPage } from './generated/MediaPreviewPage';
import type { MediaEdit } from './generated/MediaEdit';
import type { MediaEditResult } from './generated/MediaEditResult';
import type { Connection } from '../requests/connection';
import { ReadQueue } from '../requests/read-queue.ts';

export const previewReadState = () => ({
	page: null as MediaPreviewPage | null,
	loading: false,
	error: ''
});
export class MediaPreviewReader<S extends Connection> {
	private reads = new ReadQueue();
	private disposed = false;
	private current: (() => boolean) | null = null;
	private displayed: (() => boolean) | null = null;
	private session: () => S | null;
	readonly state: ReturnType<typeof previewReadState>;
	private invoke: (session: S, edit: MediaEdit) => Promise<MediaEditResult>;
	constructor(
		session: () => S | null,
		state: ReturnType<typeof previewReadState>,
		invoke: (session: S, edit: MediaEdit) => Promise<MediaEditResult>
	) {
		this.session = session;
		this.state = state;
		this.invoke = invoke;
	}
	get page() {
		const page = this.state.page;
		return this.displayed?.() ? page : null;
	}
	refresh(query: MediaPreviewQuery) {
		// Keep the last valid page in this document while a new selection resolves. The
		// existing native view can then replace its source without unregistering its frame.
		const retain = this.displayed?.() && this.state.page?.document_matches;
		this.reads.invalidate();
		this.current = null;
		if (!retain) this.state.page = null;
		this.displayed = null;
		this.state.loading = false;
		this.state.error = '';
		const session = this.session();
		if (this.disposed || !session || session.status !== 'connected' || session.loading.visible)
			return;
		const generation = session.snapshotGeneration,
			history = session.currentHistoryStateId;
		// Replies require the exact history/selection request. Displayed content has a
		// separate document fence; reconnects and project replacement still hide it at once.
		const displayed = () =>
			!this.disposed &&
			this.session() === session &&
			session.status === 'connected' &&
			!session.loading.visible &&
			session.snapshotGeneration === generation;
		this.displayed = displayed;
		if (session.hasActiveEditSession) return;
		const current = () =>
			this.current === current &&
			displayed() &&
			!session.hasActiveEditSession &&
			session.currentHistoryStateId === history;
		this.current = current;
		this.state.loading = true;
		this.reads.request(
			current,
			() => this.invoke(session, { kind: 'InspectMediaPreview', query }),
			(result) => {
				if (!current()) return;
				if (result.kind !== 'MediaPreview') {
					this.state.error = 'Unexpected preview response';
					return;
				}
				this.state.page = result.page;
			},
			(error) => {
				if (current()) this.state.error = String(error);
			},
			() => {
				if (current()) this.state.loading = false;
			}
		);
	}
	dispose() {
		this.disposed = true;
		this.current = null;
		this.displayed = null;
		this.reads.invalidate();
		this.state.page = null;
		this.state.loading = false;
	}
}
