import type { ScreenEdit } from './generated/ScreenEdit';
import type { ScreenEditResult } from './generated/ScreenEditResult';
import type { ScreenId } from './generated/ScreenId';
import type { SurfaceId } from './generated/SurfaceId';
import type { ScreenManagerSnapshot } from './generated/ScreenManagerSnapshot';
import type { MediaManagerSnapshot } from '../media/generated/MediaManagerSnapshot';
import type { SurfaceSnapshot } from './generated/SurfaceSnapshot';

interface Connection {
	readonly status: string;
	readonly snapshotGeneration: number;
	readonly loading: { readonly visible: boolean };
}

interface Client<S> {
	screens(session: S, edit: ScreenEdit): Promise<ScreenEditResult>;
	media(session: S): Promise<MediaManagerSnapshot>;
}

type OwnerInspection = Extract<ScreenEdit, { kind: 'InspectSurface' | 'InspectScreen' }>;

export interface ScreenAuthoringState {
	catalog: ScreenManagerSnapshot | null;
	media: MediaManagerSnapshot | null;
	selectedScreen: ScreenId | null;
	selectedSurface: SurfaceId | null;
	pending: boolean;
	refreshing: boolean;
	error: string;
	interacting: boolean;
}

export const createScreenAuthoringState = (): ScreenAuthoringState => ({
	catalog: null,
	media: null,
	selectedScreen: null,
	selectedSurface: null,
	pending: false,
	refreshing: false,
	error: '',
	interacting: false
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
	private ownerEpoch = 0;
	private ownerLatest: {
		session: S;
		generation: number;
		epoch: number;
		request: OwnerInspection;
	} | null = null;
	private ownerTask: Promise<void> | null = null;

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
		if (this.state.interacting) return;
		this.invalidateOwnerRead();
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

	/** Live Inspector feedback reads one owner, with one in-flight and one newest request. */
	refreshOwner(request: OwnerInspection): Promise<void> {
		if (this.disposed || this.state.interacting) return Promise.resolve();
		const session = this.getSession();
		if (!session || session.status !== 'connected' || session.loading.visible) {
			this.reset();
			return Promise.resolve();
		}
		if (
			!this.state.catalog ||
			this.context?.session !== session ||
			this.context.generation !== session.snapshotGeneration
		)
			return this.refresh();
		// A stale full-catalog reply must not replace a newer live Parameter projection.
		++this.read;
		this.state.refreshing = false;
		this.ownerLatest = {
			session,
			generation: session.snapshotGeneration,
			epoch: this.ownerEpoch,
			request
		};
		this.runOwnerRead();
		return this.ownerTask ?? Promise.resolve();
	}

	private runOwnerRead() {
		if (this.ownerTask || !this.ownerLatest) return;
		const task = this.pumpOwnerRead();
		this.ownerTask = task;
		void task.finally(() => {
			this.ownerTask = null;
			this.runOwnerRead();
		});
	}

	private async pumpOwnerRead() {
		while (this.ownerLatest) {
			const read = this.ownerLatest;
			this.ownerLatest = null;
			try {
				const result = await this.client.screens(read.session, read.request);
				if (
					!this.current(read.session, read.generation) ||
					read.epoch !== this.ownerEpoch ||
					this.state.interacting
				)
					continue;
				if (result.kind === 'Surface') this.acceptSurface(result.snapshot);
				else if (result.kind === 'ScreenDefinition') {
					const screen = this.state.catalog?.screens.find((screen) => screen.id === result.screen);
					if (screen) screen.definition = result.definition;
				} else throw new Error('Owner inspection returned an unexpected result');
				this.state.error = '';
			} catch (cause) {
				if (
					this.current(read.session, read.generation) &&
					read.epoch === this.ownerEpoch &&
					!this.state.interacting
				)
					this.state.error = String(cause);
			}
		}
	}

	private invalidateOwnerRead() {
		this.ownerEpoch++;
		this.ownerLatest = null;
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

	acceptSurface(snapshot: SurfaceSnapshot) {
		const screen = this.state.catalog?.screens.find(
			(screen) => screen.id === snapshot.reference.screen
		);
		const index =
			screen?.surfaces.findIndex(
				(surface) => surface.reference.surface === snapshot.reference.surface
			) ?? -1;
		if (screen && index >= 0) screen.surfaces[index] = snapshot;
	}

	setInteracting(active: boolean) {
		this.state.interacting = active;
		if (active) {
			++this.read;
			this.state.refreshing = false;
			this.invalidateOwnerRead();
		}
	}

	private reset() {
		++this.read;
		++this.operation;
		this.invalidateOwnerRead();
		this.context = null;
		Object.assign(this.state, createScreenAuthoringState());
	}

	dispose() {
		this.disposed = true;
		this.reset();
	}
}
