import assert from 'node:assert/strict';
import { test } from 'node:test';
import { CompositionAuthoringController, createCompositionAuthoringState } from '../authoring.ts';
import type { ChannelReference } from '../../media/generated/ChannelReference';
import type { CompositionSnapshot } from '../../media/generated/CompositionSnapshot';
import type { MediaEditResult } from '../../media/generated/MediaEditResult';

const reference = (id: string): ChannelReference => ({ media: `media-${id}`, channel: id });
const session = () => ({ status: 'connected', snapshotGeneration: 1, loading: { visible: false } });
const deferred = <T>() => {
	let resolve!: (value: T) => void;
	let reject!: (cause: unknown) => void;
	const promise = new Promise<T>((yes, no) => {
		resolve = yes;
		reject = no;
	});
	return { promise, resolve, reject };
};
const catalog = (...ids: string[]): MediaEditResult => ({
	kind: 'Manager',
	snapshot: {
		assets: [],
		media: ids.map((id) => ({
			id: reference(id).media,
			label: id,
			media_kind: 'Composition',
			channels: [
				{
					reference: reference(id),
					label: id,
					definition: {
						source: { kind: 'Composition', canvas: { width: 8, height: 4 } },
						size: { kind: 'Automatic' },
						filter: 'Linear'
					}
				}
			]
		}))
	}
});
const composition = (id: string, ...instances: string[]): MediaEditResult => ({
	kind: 'Composition',
	snapshot: {
		reference: reference(id),
		canvas: { width: 8, height: 4 },
		instances: instances.map((instance) => ({
			reference: { composition: reference(id), instance },
			label: instance,
			definition: {
				source: reference('source'),
				transform: { position: [4, 2], scale: [-2, 3], rotation: 1, pivot: [0.2, 0.7] },
				opacity: 0.6,
				blend: 'Screen',
				enabled: true
			}
		}))
	}
});

test('late catalog and Composition reads cannot replace a newer selection', async () => {
	const current = session();
	const view = createCompositionAuthoringState();
	const old = deferred<MediaEditResult>();
	const controller = new CompositionAuthoringController(
		() => current,
		view,
		async (_, edit) => {
			if (edit.kind === 'InspectManager') return catalog('a', 'b');
			if (edit.kind === 'Composition')
				return edit.reference.channel === 'a' ? old.promise : composition('b', 'b1');
			throw new Error('Unexpected mutation');
		}
	);
	const first = controller.refresh();
	await Promise.resolve();
	await controller.select(reference('b'));
	old.resolve(composition('a', 'a1'));
	await first;
	assert.equal(view.selected?.channel, 'b');
	assert.equal(view.composition?.reference.channel, 'b');
	assert.equal(view.instance, 'b1');
	assert.equal(controller.busy, false);
});

test('only the latest overlapping catalog refresh is applied', async () => {
	const current = session();
	const view = createCompositionAuthoringState();
	const old = deferred<MediaEditResult>();
	let reads = 0;
	const controller = new CompositionAuthoringController(
		() => current,
		view,
		async (_, edit) => {
			if (edit.kind === 'InspectManager') return ++reads === 1 ? old.promise : catalog('new');
			return composition('new');
		}
	);
	const first = controller.refresh();
	await controller.refresh();
	old.resolve(catalog('old'));
	await first;
	assert.equal(view.selected?.channel, 'new');
	assert.equal(view.catalog?.media[0].label, 'new');
});

test('project replacement, disconnect, loading and disposal fence old reads and errors', async () => {
	for (const mode of ['replacement', 'disconnect', 'loading', 'dispose']) {
		const current = session();
		const view = createCompositionAuthoringState();
		const reply = deferred<MediaEditResult>();
		const controller = new CompositionAuthoringController(
			() => current,
			view,
			() => reply.promise
		);
		const read = controller.refresh();
		if (mode === 'replacement') current.snapshotGeneration++;
		else if (mode === 'disconnect') current.status = 'disconnected';
		else if (mode === 'loading') current.loading.visible = true;
		else controller.dispose();
		reply.reject(new Error('old error'));
		await read;
		assert.equal(view.catalog, null);
		assert.equal(view.error, '');
		assert.equal(view.refreshing, false);
	}
});

test('backend-created identities select once; pending notifications cannot duplicate a mutation', async () => {
	const current = session();
	const view = createCompositionAuthoringState();
	const reply = deferred<MediaEditResult>();
	let mutations = 0;
	let instanceIds = ['existing'];
	const controller = new CompositionAuthoringController(
		() => current,
		view,
		async (_, edit) => {
			if (edit.kind === 'InspectManager') return catalog('a');
			if (edit.kind === 'Composition' && edit.edit.kind === 'Inspect')
				return composition('a', ...instanceIds);
			mutations++;
			assert.deepEqual(edit, {
				kind: 'Composition',
				reference: reference('a'),
				edit: { kind: 'AddInstances', sources: [reference('source')] }
			});
			return reply.promise;
		}
	);
	await controller.refresh();
	const operation = controller.edit({ kind: 'AddInstances', sources: [reference('source')] });
	await controller.edit({ kind: 'AddInstances', sources: [reference('source')] });
	await controller.refresh();
	assert.equal(mutations, 1);
	instanceIds = ['existing', 'backend-created'];
	reply.resolve({
		kind: 'InstancesCreated',
		instances: [{ composition: reference('a'), instance: 'backend-created' }]
	});
	await operation;
	assert.equal(view.instance, 'backend-created');
	assert.equal(view.pending, false);
});

test('project replacement fences mutation replies and rejects old created identities', async () => {
	const current = session();
	const view = createCompositionAuthoringState();
	const reply = deferred<MediaEditResult>();
	const controller = new CompositionAuthoringController(
		() => current,
		view,
		async (_, edit) => {
			if (edit.kind === 'InspectManager') return catalog(`project-${current.snapshotGeneration}`);
			if (edit.kind === 'Composition') return composition(edit.reference.channel);
			return reply.promise;
		}
	);
	await controller.refresh();
	const operation = controller.create();
	current.snapshotGeneration++;
	await controller.refresh();
	reply.resolve({ kind: 'Created', reference: reference('old-created') });
	await operation;
	assert.equal(view.selected?.channel, 'project-2');
	assert.equal(view.pending, false);
});

test('rejected cycles stay visible and deleted sources remain authored; removed selection reconciles', async () => {
	const current = session();
	const view = createCompositionAuthoringState();
	let ids = ['first', 'second'];
	const snapshot = (): CompositionSnapshot => {
		const result = composition('a', ...ids);
		assert.equal(result.kind, 'Composition');
		return result.snapshot;
	};
	const controller = new CompositionAuthoringController(
		() => current,
		view,
		async (_, edit) => {
			if (edit.kind === 'InspectManager') return catalog('a');
			if (edit.kind === 'Composition' && edit.edit.kind === 'Inspect')
				return { kind: 'Composition', snapshot: snapshot() };
			throw new Error('dependency cycle');
		}
	);
	await controller.refresh();
	const authored = structuredClone(view.composition);
	await controller.edit({
		kind: 'ConfigureInstance',
		instance: 'first',
		definition: { ...controller.instance!.definition, source: reference('a') }
	});
	assert.match(view.error, /dependency cycle/);
	assert.deepEqual(view.composition, authored);
	assert.deepEqual(controller.instance?.definition.source, reference('source'));
	ids = ['second'];
	await controller.refresh();
	assert.equal(view.instance, 'second');
	assert.equal(controller.busy, false);
});
