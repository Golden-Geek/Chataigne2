import type { ScreenRowSnapshot } from './generated/ScreenRowSnapshot';
interface Connection {
	readonly status: string;
	readonly snapshotGeneration: number;
	readonly loading: { readonly visible: boolean };
}
export const createRowState = () => ({ snapshot: null as ScreenRowSnapshot | null, error: '' });

/** One in-flight bounded read plus one latest request; session/project/disposal fence every reply. */
export class ScreenRowReader<S extends Connection> {
	private epoch = 0;
	private disposed = false;
	private running = false;
	private desired: { session: S; generation: number; node: string; epoch: number } | null = null;
	private state: ReturnType<typeof createRowState>;
	private read: (session: S, node: string) => Promise<ScreenRowSnapshot>;
	constructor(
		state: ReturnType<typeof createRowState>,
		read: (session: S, node: string) => Promise<ScreenRowSnapshot>
	) {
		this.state = state;
		this.read = read;
	}
	refresh(session: S | null, node: string) {
		if (this.disposed) return;
		const epoch = ++this.epoch;
		if (session?.status !== 'connected' || session.loading.visible) {
			this.desired = null;
			this.state.snapshot = null;
			this.state.error = '';
			return;
		}
		if (
			this.desired?.session !== session ||
			this.desired.generation !== session.snapshotGeneration ||
			this.desired.node !== node
		)
			this.state.snapshot = null;
		this.desired = { session, generation: session.snapshotGeneration, node, epoch };
		if (!this.running) void this.drain();
	}
	private async drain() {
		this.running = true;
		try {
			while (!this.disposed && this.desired) {
				const request = this.desired;
				const current = () =>
					!this.disposed &&
					request.epoch === this.epoch &&
					request.session.status === 'connected' &&
					!request.session.loading.visible &&
					request.session.snapshotGeneration === request.generation;
				try {
					const snapshot = await this.read(request.session, request.node);
					if (current()) {
						this.state.snapshot = snapshot;
						this.state.error = '';
					}
				} catch (error) {
					if (current()) {
						this.state.snapshot = null;
						this.state.error = String(error);
					}
				}
				if (request.epoch === this.epoch) break;
			}
		} finally {
			this.running = false;
		}
	}
	dispose() {
		this.disposed = true;
		this.epoch++;
		this.desired = null;
		this.state.snapshot = null;
	}
}
