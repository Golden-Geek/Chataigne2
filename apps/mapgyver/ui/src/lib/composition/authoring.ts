import { channelKey } from '../media/identity.ts';
import type { ChannelReference } from '../media/generated/ChannelReference';
import type { ChannelDefinition } from '../media/generated/ChannelDefinition';
import type { CompositionEdit } from '../media/generated/CompositionEdit';
import type { CompositionSnapshot } from '../media/generated/CompositionSnapshot';
import type { InstanceId } from '../media/generated/InstanceId';
import type { MediaEdit } from '../media/generated/MediaEdit';
import type { MediaEditResult } from '../media/generated/MediaEditResult';
import type { MediaManagerSnapshot } from '../media/generated/MediaManagerSnapshot';

interface Connection {
	readonly status: string;
	readonly snapshotGeneration: number;
	readonly loading: { readonly visible: boolean };
}

export interface CompositionAuthoringState {
	catalog: MediaManagerSnapshot | null;
	composition: CompositionSnapshot | null;
	selected: ChannelReference | null;
	instance: InstanceId | null;
	pending: boolean;
	refreshing: boolean;
	error: string;
}

export const createCompositionAuthoringState = (): CompositionAuthoringState => ({
	catalog: null,
	composition: null,
	selected: null,
	instance: null,
	pending: false,
	refreshing: false,
	error: ''
});

/** Presentation selection and request fencing; all authored behavior stays in backend intents. */
export class CompositionAuthoringController<S extends Connection> {
	private read = 0;
	private operation = 0;
	private disposed = false;
	private context: { session: S; generation: number } | null = null;
	private getSession: () => S | null;
	private state: CompositionAuthoringState;
	private invoke: (session: S, edit: MediaEdit) => Promise<MediaEditResult>;

	constructor(
		getSession: () => S | null,
		state: CompositionAuthoringState,
		invoke: (session: S, edit: MediaEdit) => Promise<MediaEditResult>
	) {
		this.getSession = getSession;
		this.state = state;
		this.invoke = invoke;
	}

	get channels() {
		return (
			this.state.catalog?.media.flatMap((media) =>
				media.channels.filter((channel) => channel.definition.source.kind === 'Composition')
			) ?? []
		);
	}
	get channel() {
		const selected = this.state.selected;
		return selected
			? this.channels.find((channel) => channelKey(channel.reference) === channelKey(selected))
			: undefined;
	}
	get instance() {
		return this.state.composition?.instances.find(
			(instance) => instance.reference.instance === this.state.instance
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
		const session = this.getSession();
		if (!session || session.status !== 'connected' || session.loading.visible) {
			this.reset();
			return;
		}
		const generation = session.snapshotGeneration;
		if (this.context?.session !== session || this.context.generation !== generation) this.reset();
		this.context = { session, generation };
		// A committed edit will inspect once after its response, including external changes.
		if (this.state.pending) return;
		const read = ++this.read;
		this.state.refreshing = true;
		try {
			const catalog = await this.invoke(session, { kind: 'InspectManager' });
			if (read !== this.read || !this.current(session, generation)) return;
			if (catalog.kind !== 'Manager') throw new Error('Unexpected media inspection result');
			this.state.catalog = catalog.snapshot;
			if (!this.channel) {
				this.state.selected = this.channels[0]?.reference ?? null;
				this.state.instance = null;
				this.state.composition = null;
			}
			const reference = this.state.selected;
			if (!reference) return;
			const result = await this.invoke(session, {
				kind: 'Composition',
				reference,
				edit: { kind: 'Inspect' }
			});
			if (read !== this.read || !this.current(session, generation)) return;
			if (result.kind !== 'Composition')
				throw new Error('Unexpected Composition inspection result');
			this.state.composition = result.snapshot;
			if (!this.instance)
				this.state.instance = result.snapshot.instances[0]?.reference.instance ?? null;
		} catch (cause) {
			if (read === this.read && this.current(session, generation)) this.state.error = String(cause);
		} finally {
			if (read === this.read) this.state.refreshing = false;
		}
	}

	select(reference: ChannelReference) {
		if (this.disposed || this.state.pending) return;
		this.state.selected = reference;
		this.state.instance = null;
		this.state.composition = null;
		this.state.error = '';
		return this.refresh();
	}
	create() {
		return this.mutate({ kind: 'CreateMedia', media_kind: 'Composition', label: null });
	}
	edit(edit: CompositionEdit) {
		const reference = this.state.selected;
		if (reference) return this.mutate({ kind: 'Composition', reference, edit });
	}
	configure(definition: ChannelDefinition) {
		const reference = this.state.selected;
		if (reference) return this.mutate({ kind: 'ConfigureChannel', reference, definition });
	}

	private async mutate(edit: MediaEdit): Promise<MediaEditResult | undefined> {
		const session = this.getSession();
		if (!session || this.busy) return;
		const generation = session.snapshotGeneration;
		const operation = ++this.operation;
		++this.read;
		this.state.pending = true;
		this.state.error = '';
		try {
			const result = await this.invoke(session, edit);
			if (operation !== this.operation || !this.current(session, generation)) return;
			if (result.kind === 'Created') {
				this.state.selected = result.reference;
				this.state.instance = null;
				this.state.composition = null;
			} else if (result.kind === 'InstancesCreated' && result.instances[0]) {
				this.state.instance = result.instances[0].instance;
			}
			return result;
		} catch (cause) {
			if (operation === this.operation && this.current(session, generation))
				this.state.error = String(cause);
		} finally {
			if (operation === this.operation) {
				this.state.pending = false;
				if (this.current(session, generation)) await this.refresh();
			}
		}
	}
	private reset() {
		++this.read;
		++this.operation;
		this.context = null;
		Object.assign(this.state, createCompositionAuthoringState());
	}
	dispose() {
		this.disposed = true;
		this.reset();
	}
}
