import type { ScreenEdit } from './generated/ScreenEdit';
import type { ScreenEditResult } from './generated/ScreenEditResult';
import type { ScreenId } from './generated/ScreenId';
import type { SurfaceId } from './generated/SurfaceId';
import type { ScreenManagerSnapshot } from './generated/ScreenManagerSnapshot';
import type { MediaManagerSnapshot } from '../media/generated/MediaManagerSnapshot';

interface Connection {
	readonly status: string;
	readonly snapshotGeneration: number;
	readonly loading: { readonly visible: boolean };
}

interface Client<S> {
	screens(session: S, edit: ScreenEdit): Promise<ScreenEditResult>;
	media(session: S): Promise<MediaManagerSnapshot>;
}

export interface ScreenAuthoringState {
	catalog: ScreenManagerSnapshot | null;
	media: MediaManagerSnapshot | null;
	selectedScreen: ScreenId | null;
	selectedSurface: SurfaceId | null;
	pending: boolean;
	refreshing: boolean;
	error: string;
}

export const createScreenAuthoringState = (): ScreenAuthoringState => ({
	catalog: null,
	media: null,
	selectedScreen: null,
	selectedSurface: null,
	pending: false,
	refreshing: false,
	error: ''
});

/** Presentation state only. Session/project fences discard stale reads and edit replies. */
export class ScreenAuthoringController<S extends Connection> {
	private getSession: () => S | null;
	private state: ScreenAuthoringState;
	private client: Client<S>;
	private read = 0;
	private operation = 0;
	private disposed = false;
	private context: { session: S; generation: number } | null = null;

	constructor(getSession: () => S | null, state: ScreenAuthoringState, client: Client<S>) {
		this.getSession = getSession;
		this.state = state;
		this.client = client;
	}

	get screen() {
		return this.state.catalog?.screens.find((screen) => screen.id === this.state.selectedScreen);
	}
	get surface() {
		return this.screen?.surfaces.find(
			(surface) => surface.reference.surface === this.state.selectedSurface
		);
	}
	get busy() {
		const session = this.getSession();
		return (
			this.disposed ||
			this.state.pending ||
			this.state.refreshing ||
			!this.state.catalog ||
			session?.status !== 'connected' ||
			session.loading.visible
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

	async refresh() {
		if (this.disposed) return;
		++this.read;
		const session = this.getSession();
		if (!session || session.status !== 'connected' || session.loading.visible) {
			this.reset();
			return;
		}
		const generation = session.snapshotGeneration;
		if (this.context?.session !== session || this.context.generation !== generation) this.reset();
		this.context = { session, generation };
		this.state.refreshing = true;
		const active = this.read;
		try {
			const [result, media] = await Promise.all([
				this.client.screens(session, { kind: 'InspectManager' }),
				this.client.media(session)
			]);
			if (active !== this.read || !this.current(session, generation)) return;
			if (result.kind !== 'Manager')
				throw new Error('Screen inspection returned an unexpected result');
			this.state.catalog = result.snapshot;
			this.state.media = media;
			if (!this.screen) this.state.selectedScreen = result.snapshot.screens[0]?.id ?? null;
			if (!this.surface)
				this.state.selectedSurface = this.screen?.surfaces[0]?.reference.surface ?? null;
			this.state.error = '';
		} catch (cause) {
			if (active === this.read && this.current(session, generation))
				this.state.error = String(cause);
		} finally {
			if (active === this.read) this.state.refreshing = false;
		}
	}

	async edit(edit: ScreenEdit): Promise<ScreenEditResult | undefined> {
		const session = this.getSession();
		if (!session || this.busy) return;
		const generation = session.snapshotGeneration;
		const operation = ++this.operation;
		this.state.pending = true;
		this.state.error = '';
		try {
			const result = await this.client.screens(session, edit);
			if (operation !== this.operation || !this.current(session, generation)) return;
			if (result.kind === 'ScreenCreated') {
				this.state.selectedScreen = result.screen;
				this.state.selectedSurface = result.surface.surface;
			} else if (result.kind === 'SurfaceCreated') {
				this.state.selectedScreen = result.reference.screen;
				this.state.selectedSurface = result.reference.surface;
			}
			await this.refresh();
			return result;
		} catch (cause) {
			if (operation === this.operation && this.current(session, generation))
				this.state.error = String(cause);
		} finally {
			if (operation === this.operation) this.state.pending = false;
		}
	}

	selectScreen(screen: ScreenId) {
		this.state.selectedScreen = screen;
		this.state.selectedSurface = this.screen?.surfaces[0]?.reference.surface ?? null;
	}

	private reset() {
		++this.read;
		++this.operation;
		this.context = null;
		Object.assign(this.state, createScreenAuthoringState());
	}

	dispose() {
		this.disposed = true;
		this.reset();
	}
}
