import type { SpatialEditorQuery } from '../media/generated/SpatialEditorQuery';
import type { SpatialEditorBinding } from '../media/generated/SpatialEditorBinding';
import type { MediaEdit } from '../media/generated/MediaEdit';
import type { MediaEditResult } from '../media/generated/MediaEditResult';
import type { Connection } from '../requests/connection';
import { ReadQueue } from '../requests/read-queue.ts';
import { targetKey } from './targets.ts';
interface BindingConnection extends Connection {
	readonly projectGeneration: number | null;
}
export const bindingState = () => ({
	binding: null as SpatialEditorBinding | null,
	refreshing: false,
	error: ''
});
/** One panel's target discovery; focused domain readers own their projections and authoring. */
export class SpatialBindingReader<S extends BindingConnection> {
	private reads = new ReadQueue();
	private disposed = false;
	private context: { session: S; project: number } | null = null;
	private key = '';
	private session: () => S | null;
	readonly state: ReturnType<typeof bindingState>;
	private invoke: (session: S, edit: MediaEdit) => Promise<MediaEditResult>;
	constructor(
		session: () => S | null,
		state: ReturnType<typeof bindingState>,
		invoke: (session: S, edit: MediaEdit) => Promise<MediaEditResult>
	) {
		this.session = session;
		this.state = state;
		this.invoke = invoke;
	}
	get binding() {
		const binding = this.state.binding,
			session = this.session();
		return !this.disposed &&
			session &&
			session === this.context?.session &&
			session.projectGeneration !== null &&
			session.projectGeneration === this.context.project
			? binding
			: null;
	}
	refresh(query: SpatialEditorQuery) {
		if (this.disposed) return;
		const session = this.session();
		this.reads.invalidate();
		if (
			this.context &&
			(this.context.session !== session || this.context.project !== session?.projectGeneration)
		) {
			this.context = null;
			this.state.binding = null;
			this.state.error = '';
		}
		if (
			!session ||
			session.status !== 'connected' ||
			session.loading.visible ||
			session.projectGeneration === null
		) {
			this.state.refreshing = false;
			return;
		}
		const generation = session.snapshotGeneration,
			project = session.projectGeneration,
			history = session.currentHistoryStateId;
		const key = JSON.stringify([
			query.selected,
			query.document,
			targetKey(query.locked),
			targetKey(query.retained)
		]);
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
			() => this.invoke(session, { kind: 'InspectSpatialEditor', query }),
			(result) => {
				if (!current()) return;
				if (result.kind !== 'SpatialEditor') {
					this.state.error = 'Unexpected spatial binding response';
					return;
				}
				this.context = { session, project };
				this.state.binding = result.binding;
				this.state.error = '';
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
