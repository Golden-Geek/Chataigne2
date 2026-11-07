// Prepare a finite owned manual scene using public authoring APIs; inject no desktop input.
import assert from 'node:assert/strict';
import { mkdir, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';

const directory = path.resolve(process.argv[2]);
await mkdir(directory);
const native = await openNativeEditor();
let token;
async function post(route, body) {
	const response = await fetch(`${native.base}${route}`, {
		method: 'POST',
		signal: AbortSignal.timeout(5000),
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body)
	});
	const value = await response.json();
	assert(response.ok, JSON.stringify(value));
	return value;
}
async function edit(payload) {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.media',
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}
try {
	token = (await post('/api/ui/session', {})).token;
	const source = (
		await edit({ kind: 'CreateMedia', media_kind: 'SolidColor', label: 'Projection red source' })
	).reference;
	await edit({
		kind: 'ConfigureChannel',
		reference: source,
		source: { kind: 'SolidColor', rgba: [1, 0, 0, 1] }
	});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: source.media,
		size: { kind: 'Fixed', extent: { width: 120, height: 90 } }
	});
	const composition = (
		await edit({
			kind: 'CreateMedia',
			media_kind: 'Composition',
			label: 'Live Projection Composition'
		})
	).reference;
	await edit({
		kind: 'ConfigureMediaResolution',
		media: composition.media,
		size: { kind: 'Fixed', extent: { width: 400, height: 300 } }
	});
	const operation = (value) => edit({ kind: 'Composition', reference: composition, edit: value });
	const instance = (await operation({ kind: 'AddInstances', sources: [source] })).instances[0]
		.instance;
	const initial = (await operation({ kind: 'Inspect' })).snapshot.instances[0];
	await operation({
		kind: 'ConfigureInstance',
		instance,
		definition: {
			...initial.definition,
			transform: {
				position: [190, 140],
				scale: [1.6, 1.6],
				rotation_degrees: 23,
				pivot: [0.5, 0.5]
			}
		}
	});
	await post('/api/ui/project-save', { path: path.join(directory, 'manual.mapgyver') });
	const deadline = Date.now() + 8 * 60 * 1000;
	const ready = {
		pid: native.app.pid,
		composition,
		instance,
		deadline: new Date(deadline).toISOString(),
		injected_input: false,
		capture: false
	};
	await writeFile(path.join(directory, 'ready.json'), JSON.stringify(ready, null, 2));
	console.log(JSON.stringify(ready));
	while (Date.now() < deadline && native.app.exitCode === null) {
		if (
			await stat(path.join(directory, 'review.done')).then(
				() => true,
				() => false
			)
		)
			break;
		await delay(1000);
	}
} finally {
	await writeFile(path.join(directory, 'host.log'), native.log());
	await native.close();
}
