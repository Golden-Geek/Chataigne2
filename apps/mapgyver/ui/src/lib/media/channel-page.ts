import type { MediaChannelsPage } from './generated/MediaChannelsPage';
import type { MediaEdit } from './generated/MediaEdit';
import type { MediaEditResult } from './generated/MediaEditResult';
import type { Connection } from '../requests/connection';
import { ReadQueue } from '../requests/read-queue.ts';

export const CHANNEL_PAGE_SIZE = 32;
export const channelPageState = () => ({
	page: null as MediaChannelsPage | null,
	loading: false,
	error: ''
});

/** A bounded authored read; selection, project and history changes retire obsolete replies. */
export class ChannelPageReader<S extends Connection> {
	private reads = new ReadQueue();
	private disposed = false;
	private current: (() => boolean) | null = null;
	private session: () => S | null;
	readonly state: ReturnType<typeof channelPageState>;
	private invoke: (session: S, edit: MediaEdit) => Promise<MediaEditResult>;
	constructor(
		session: () => S | null,
		state: ReturnType<typeof channelPageState>,
		invoke: (session: S, edit: MediaEdit) => Promise<MediaEditResult>
	) {
		this.session = session;
		this.state = state;
		this.invoke = invoke;
	}
	get page() {
		const page = this.state.page;
		return this.current?.() ? page : null;
	}
	refresh(media: string, start: number) {
		this.reads.invalidate();
		this.current = null;
		this.state.page = null;
		this.state.error = '';
		this.state.loading = false;
		const session = this.session();
		if (
			this.disposed ||
			!session ||
			session.status !== 'connected' ||
			session.loading.visible ||
			session.hasActiveEditSession
		)
			return;
		const generation = session.snapshotGeneration;
		const history = session.currentHistoryStateId;
		const current = () =>
			!this.disposed &&
			this.current === current &&
			this.session() === session &&
			session.status === 'connected' &&
			!session.loading.visible &&
			!session.hasActiveEditSession &&
			session.snapshotGeneration === generation &&
			session.currentHistoryStateId === history;
		this.current = current;
		this.state.loading = true;
		this.reads.request(
			current,
			() =>
				this.invoke(session, {
					kind: 'InspectChannels',
					query: { media, start, count: CHANNEL_PAGE_SIZE }
				}),
			(result) => {
				if (!current()) return;
				if (
					result.kind !== 'Channels' ||
					result.page.media !== media ||
					result.page.start !== start
				) {
					this.state.error = 'Unexpected channel page';
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
		this.reads.invalidate();
		this.state.page = null;
		this.state.loading = false;
	}
}
