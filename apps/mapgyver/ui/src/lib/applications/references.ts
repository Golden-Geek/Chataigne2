import type { ApplicationReference } from '../media/generated/ApplicationReference';
import { ReadQueue } from '../requests/read-queue.ts';
import type { Invoke } from './catalog';
import type { Connection } from '../requests/connection';

export const referenceState = () => ({
	references: [] as ApplicationReference[],
	loading: false,
	error: ''
});
export class ApplicationReferences<S extends Connection> {
	private reads = new ReadQueue();
	private disposed = false;
	private session: () => S | null;
	private identities: () => string[];
	readonly state: ReturnType<typeof referenceState>;
	private invoke: Invoke<S>;
	constructor(
		session: () => S | null,
		identities: () => string[],
		state: ReturnType<typeof referenceState>,
		invoke: Invoke<S>
	) {
		this.session = session;
		this.identities = identities;
		this.state = state;
		this.invoke = invoke;
	}
	refresh() {
		const session = this.session();
		if (this.disposed) return;
		if (!session || session.status !== 'connected' || session.loading.visible) {
			this.reset();
			return;
		}
		if (session.hasActiveEditSession) return;
		const generation = session.snapshotGeneration,
			history = session.currentHistoryStateId,
			applications = [...this.identities()];
		const current = () =>
			!this.disposed &&
			this.session() === session &&
			session.status === 'connected' &&
			!session.loading.visible &&
			session.snapshotGeneration === generation &&
			session.currentHistoryStateId === history &&
			JSON.stringify(this.identities()) === JSON.stringify(applications);
		this.state.loading = true;
		this.reads.request(
			() =>
				this.invoke(session, {
					kind: 'Applications',
					edit: { kind: 'InspectReferences', applications }
				}),
			(result) => {
				if (!current()) return;
				if (result.kind !== 'ApplicationReferences') {
					this.state.references = [];
					this.state.error = 'Unexpected application references';
					return;
				}
				this.state.references = result.references;
				this.state.error = '';
			},
			(error) => {
				if (current()) {
					this.state.references = [];
					this.state.error = String(error);
				}
			},
			() => {
				this.state.loading = false;
			}
		);
	}
	reset() {
		this.reads.invalidate();
		this.state.references = [];
		this.state.loading = false;
		this.state.error = '';
	}
	dispose() {
		this.disposed = true;
		this.reset();
	}
}
