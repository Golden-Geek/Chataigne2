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
interface PreviewConnection extends Connection {
	readonly projectGeneration: number | null;
}

export class MediaPreviewReader<S extends PreviewConnection> {
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
		// The native renderer outlives the graph transport. Preserve its valid presentation
		// through selection, reconnect and same-project snapshots, independently of reads.
		const retain = this.displayed?.() && this.state.page?.document_matches;
		this.reads.invalidate();
		this.current = null;
		if (!retain) this.state.page = null;
		if (!retain) this.displayed = null;
		this.state.loading = false;
		this.state.error = '';
		const session = this.session();
		if (
			this.disposed ||
			!session ||
			session.status !== 'connected' ||
			session.loading.visible ||
			session.projectGeneration === null
		)
			return;
		const generation = session.snapshotGeneration,
			project = session.projectGeneration,
			history = session.currentHistoryStateId;
		// Project replacement/loading still fences the old view. A snapshot refresh or
		// lost graph socket alone does not end the backend's project/native-view lifetime.
		const displayed = () =>
			!this.disposed &&
			this.session() === session &&
			!session.loading.visible &&
			session.projectGeneration === project;
		this.displayed = displayed;
		if (session.hasActiveEditSession) return;
		const current = () =>
			this.current === current &&
			displayed() &&
			session.status === 'connected' &&
			session.snapshotGeneration === generation &&
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
