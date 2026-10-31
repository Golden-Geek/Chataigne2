import type { MatrixTargetPage } from '../media/generated/MatrixTargetPage';
import type { MatrixTargetScope } from '../media/generated/MatrixTargetScope';
import type { MatrixTargetId } from '../media/generated/MatrixTargetId';
import type { MediaEdit } from '../media/generated/MediaEdit';
import type { MediaEditResult } from '../media/generated/MediaEditResult';
import type { Connection } from './choices.ts';
import { ReadQueue } from './read-queue.ts';

export const targetKey = (id: MatrixTargetId | null) =>
	!id ? '' : id.kind === 'Column' ? `column:${id.column}` : `${id.item.kind}:${id.item.id}`;

export const targetState = () => ({
	page: null as MatrixTargetPage | null,
	search: '',
	start: 0,
	loading: false,
	error: ''
});

/** Bounded destination reads share the same session/history fences as viewport reads. */
export class TargetReader<S extends Connection> {
	private queue = new ReadQueue();
	private disposed = false;
	private session: () => S | null;
	private context: () => { media: string; scope: MatrixTargetScope };
	readonly state: ReturnType<typeof targetState>;
	private invoke: (session: S, edit: MediaEdit) => Promise<MediaEditResult>;
	constructor(
		session: () => S | null,
		context: () => { media: string; scope: MatrixTargetScope },
		state: ReturnType<typeof targetState>,
		invoke: (session: S, edit: MediaEdit) => Promise<MediaEditResult>
	) {
		this.session = session;
		this.context = context;
		this.state = state;
		this.invoke = invoke;
	}
	refresh() {
		if (this.disposed) return;
		const session = this.session();
		this.queue.invalidate();
		if (!session || session.status !== 'connected' || session.loading.visible) {
			this.state.page = null;
			this.state.loading = false;
			this.state.error = '';
			return;
		}
		if (session.hasActiveEditSession) {
			this.state.loading = false;
			return;
		}
		const context = this.context(),
			key = JSON.stringify(context),
			generation = session.snapshotGeneration,
			history = session.currentHistoryStateId;
		const valid = () =>
			!this.disposed &&
			this.session() === session &&
			session.status === 'connected' &&
			!session.loading.visible &&
			!session.hasActiveEditSession &&
			session.snapshotGeneration === generation &&
			session.currentHistoryStateId === history &&
			JSON.stringify(this.context()) === key;
		const query = {
			scope: context.scope,
			search: this.state.search,
			start: this.state.start,
			count: 64
		};
		this.state.loading = true;
		this.queue.request(
			() =>
				this.invoke(session, {
					kind: 'Matrix',
					media: context.media,
					edit: { kind: 'InspectTargets', query }
				}),
			(result) => {
				if (!valid()) return;
				if (result.kind !== 'MatrixTargets') {
					this.state.error = 'Unexpected Matrix targets';
					return;
				}
				this.state.page = result.page;
				this.state.error = '';
			},
			(error) => {
				if (valid()) this.state.error = String(error);
			},
			() => {
				this.state.loading = false;
			}
		);
	}
	dispose() {
		this.disposed = true;
		this.queue.invalidate();
		this.state.loading = false;
	}
}
