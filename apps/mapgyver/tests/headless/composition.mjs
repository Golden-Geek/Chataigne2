// M06 caller parity and a review-ready project through the existing ordinary headless host.
import assert from 'node:assert/strict';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createHash, randomUUID } from 'node:crypto';
import path from 'node:path';

export async function verifyComposition({ post, intent, edit }) {
	const operation = (reference, value) => edit({ kind: 'Composition', reference, edit: value });
	const inspect = async (reference) => (await operation(reference, { kind: 'Inspect' })).snapshot;
	const create = async (media_kind, label) =>
		(await edit({ kind: 'CreateMedia', media_kind, label })).reference;
	const red = await create('SolidColor', 'Review Red');
	const green = await create('SolidColor', 'Review Green');
	for (const [reference, rgba] of [
		[red, [1, 0.04, 0.02, 0.9]],
		[green, [0.03, 1, 0.1, 0.7]]
	]) {
		const { snapshot } = await edit({ kind: 'InspectChannel', reference });
		await edit({
			kind: 'ConfigureChannel',
			reference,
			definition: {
				...snapshot.definition,
				source: { kind: 'SolidColor', rgba },
				size: { kind: 'Fixed', extent: { width: 20, height: 16 } }
			}
		});
	}
	const root = await create('Composition', 'Review Composition');
	const { snapshot: channel } = await edit({ kind: 'InspectChannel', reference: root });
	await edit({
		kind: 'ConfigureChannel',
		reference: root,
		definition: {
			...channel.definition,
			source: { kind: 'Composition', canvas: { width: 320, height: 240 } }
		}
	});
	const { instances } = await operation(root, { kind: 'AddInstances', sources: [red, red, green] });
	const initial = await inspect(root);
	for (const [index, position] of [
		[0, [80, 90]],
		[1, [230, 150]],
		[2, [160, 120]]
	]) {
		await operation(root, {
			kind: 'ConfigureInstance',
			instance: instances[index].instance,
			definition: {
				...initial.instances[index].definition,
				transform: {
					position,
					scale: [4, 4],
					rotation: 0,
					pivot: [0.5, 0.5]
				}
			}
		});
	}
	const review = await inspect(root);
	const directory = path.resolve('target/m06-editor-review');
	await mkdir(directory, { recursive: true });
	const project = path.join(directory, 'CompositionReview.mapgyver');
	assert.equal((await post('/api/ui/project-save', { path: project })).ok, true);
	const first = instances[0].instance;
	let samples = 0;
	for (const mode of ['Translate', 'Rotate', 'Scale']) {
		const before = await inspect(root);
		const origin = before.instances[0].definition.transform;
		const [x, y] = origin.position;
		const client_edit_id = randomUUID();
		assert((await intent({ kind: 'beginEdit', client_edit_id, label: `${mode} review` })).success);
		for (let step = 1; step <= 10; step++) {
			const start =
				mode === 'Translate'
					? { x, y }
					: mode === 'Rotate'
						? { x: x + 20, y }
						: { x: x - 20, y: y + 20 };
			const point =
				mode === 'Translate'
					? { x: x + 2 * step, y: y + step }
					: mode === 'Rotate'
						? {
								x: x + 20 * Math.cos((Math.PI * step) / 20),
								y: y + 20 * Math.sin((Math.PI * step) / 20)
							}
						: { x: x - 20 * (1 - step / 20), y: y + 20 * (1 + step / 10) };
			const reply = await operation(root, {
				kind: 'EditInstanceGeometry',
				instance: first,
				edit: { mode, origin, start, point }
			});
			assert.equal(reply.kind, 'InstanceChanged');
			assert.equal(reply.snapshot.reference.instance, first);
			samples++;
		}
		assert((await intent({ kind: 'endEdit', client_edit_id })).success);
		const after = await inspect(root);
		assert.deepEqual(after.instances.slice(1), before.instances.slice(1));
		const transform = after.instances[0].definition.transform;
		if (mode === 'Translate') assert.deepEqual(transform.position, [100, 100]);
		if (mode === 'Rotate') assert(Math.abs(transform.rotation - Math.PI / 2) < 1e-6);
		if (mode === 'Scale') {
			assert(Math.abs(transform.scale[0] - 8) < 1e-5);
			assert(Math.abs(transform.scale[1] - 2) < 1e-5);
		}
		assert((await intent({ kind: 'undo' })).success);
		assert.deepEqual(await inspect(root), before, `${mode} samples must be one undo`);
		assert((await intent({ kind: 'redo' })).success);
		assert.deepEqual(await inspect(root), after);
	}
	const beforeSource = await inspect(root);
	await operation(root, {
		kind: 'ConfigureInstance',
		instance: first,
		definition: { ...beforeSource.instances[0].definition, source: green }
	});
	assert.deepEqual((await inspect(root)).instances[1], beforeSource.instances[1]);
	assert.deepEqual((await inspect(root)).instances[0].definition.source, green);
	await operation(root, {
		kind: 'RenameInstance',
		instance: first,
		label: 'Headless edited instance'
	});
	await operation(root, { kind: 'ReorderInstance', instance: first, after: instances[2].instance });
	assert.equal((await inspect(root)).instances.at(-1).reference.instance, first);
	const { instances: copies } = await operation(root, {
		kind: 'DuplicateInstances',
		instances: [first]
	});
	assert.equal(copies.length, 1);
	assert.notEqual(copies[0].instance, first);
	await operation(root, { kind: 'RemoveInstance', instance: copies[0].instance });
	const beforeReject = await inspect(root);
	const rejected = await intent({
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.media',
		operation: 'edit',
		payload: {
			kind: 'Composition',
			reference: root,
			edit: { kind: 'AddInstances', sources: [root] }
		}
	});
	assert.equal(rejected.success, false);
	assert.deepEqual(await inspect(root), beforeReject);
	await edit({ kind: 'RemoveMedia', media: red.media });
	assert.deepEqual((await inspect(root)).instances[0].definition.source, red);
	assert((await intent({ kind: 'undo' })).success);
	assert.deepEqual(await inspect(root), beforeReject);
	assert.equal((await post('/api/ui/project-new', {})).ok, true);
	await post('/api/ui/project-load', { path: project });
	assert.deepEqual(await inspect(root), review);
	const report = {
		result: 'pass',
		scope: 'ordinary headless Composition intents; real editor gestures remain unverified',
		project,
		project_sha256: createHash('sha256')
			.update(await readFile(project))
			.digest('hex'),
		root,
		red,
		green,
		instances,
		grouped_geometry_samples: samples,
		grouped_undo_redo_modes: 3,
		original_scene_restored: true
	};
	await writeFile(path.join(directory, 'review.json'), JSON.stringify(report, null, 2));
	console.log(JSON.stringify(report));
}
