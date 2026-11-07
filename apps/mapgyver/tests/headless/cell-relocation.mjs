// Ordinary host/HTTP contract; this does not measure native pixels or input cadence.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { openInitialScreenHeadless } from './initial-screen-host.mjs';

const directory = path.resolve(process.argv[2]);
await mkdir(directory);
assert(process.env.MAPGYVER_TEST_BINARY);
const host = await openInitialScreenHeadless(directory);
let token;
const checks = [];
async function post(route, body) {
	const response = await fetch(host.base + route, {
		method: 'POST',
		signal: AbortSignal.timeout(5000),
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body)
	});
	const result = await response.json();
	assert(response.ok, JSON.stringify(result));
	return result;
}
const intent = (value) => post('/api/ui/intent', value);
async function operation(payload) {
	const ack = await intent({
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.media',
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return { value: ack.result.value, history: ack.history };
}
try {
	token = (await post('/api/ui/session', {})).token;
	const reference = (
		await operation({ kind: 'CreateMedia', media_kind: 'Matrix', label: 'Cell relocation' })
	).value.reference;
	const matrix = (edit) => operation({ kind: 'Matrix', media: reference.media, edit });
	const read = async () => {
		const result = await matrix({ kind: 'Inspect' });
		return { value: result.value.snapshot, history: result.history };
	};
	const initial = (await read()).value;
	assert.equal(initial.items.length, 3);
	assert.equal(initial.columns.length, 3);
	const rows = initial.items.map((item) => item.id.id),
		columns = initial.columns.map((column) => column.id);
	const a = (
		await matrix({
			kind: 'CreateCellMedia',
			layer: rows[0],
			column: columns[0],
			media_kind: 'SolidColor'
		})
	).value.cell;
	await matrix({
		kind: 'CreateCellMedia',
		layer: rows[1],
		column: columns[1],
		media_kind: 'SolidColor'
	});
	await matrix({ kind: 'EnsureCell', layer: rows[0], column: columns[1] });
	for (const [layer, column, occupied] of [
		[rows[0], columns[2], 'Reject'],
		[rows[1], columns[1], 'Swap'],
		[rows[0], columns[2], 'Swap'],
		[rows[0], columns[1], 'Swap']
	]) {
		const before = await read();
		const source = before.value.cells.find((cell) => cell.id === a);
		await matrix({
			kind: 'TransferCell',
			cell: a,
			layer,
			column,
			action: occupied === 'Swap' ? 'Drop' : 'Move'
		});
		const after = await read();
		const moved = after.value.cells.find((cell) => cell.id === a);
		assert.equal(moved.layer, layer);
		assert.equal(moved.definition.column, column);
		assert.deepEqual(moved.media, source.media);
		assert.deepEqual(moved.definition.settings, source.definition.settings);
		assert.equal(after.value.cells.length, before.value.cells.length);
		assert.equal(after.history.undo_len, before.history.undo_len + 1);
		assert((await intent({ kind: 'undo' })).success);
		assert.deepEqual((await read()).value, before.value);
		assert((await intent({ kind: 'redo' })).success);
		assert.deepEqual((await read()).value, after.value);
		checks.push({ layer, column, occupied, identity: true, exact_history: true });
	}
	const before = await read();
	const rejected = await intent({
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.media',
		operation: 'edit',
		payload: {
			kind: 'Matrix',
			media: reference.media,
			edit: {
				kind: 'TransferCell',
				cell: a,
				layer: rows[1],
				column: columns[1],
				action: 'Move'
			}
		}
	});
	assert.equal(rejected.success, false);
	assert.deepEqual((await read()).value, before.value);
	assert.equal((await read()).history.undo_len, before.history.undo_len);
	checks.push({ occupied_rejection: true, no_partial_state: true, exact_history: true });
	await matrix({
		kind: 'TransferCell',
		cell: a,
		layer: rows[0],
		column: columns[1],
		action: 'Move'
	});
	assert.deepEqual((await read()).value, before.value);
	assert.equal((await read()).history.undo_len, before.history.undo_len);
	checks.push({ same_address: true, exact_noop: true });
	const other = (
		await operation({
			kind: 'CreateMedia',
			media_kind: 'Matrix',
			label: 'Cell duplicate destination'
		})
	).value.reference;
	const otherSnapshot = (
		await operation({ kind: 'Matrix', media: other.media, edit: { kind: 'Inspect' } })
	).value.snapshot;
	for (const [targetMedia, layer, column, mode] of [
		[reference.media, rows[2], columns[0], 'Copy'],
		[reference.media, rows[2], columns[1], 'Reference'],
		[other.media, otherSnapshot.items[0].id.id, otherSnapshot.columns[0].id, 'Copy'],
		[other.media, otherSnapshot.items[0].id.id, otherSnapshot.columns[1].id, 'Reference']
	]) {
		const inspect = async () => {
			const result = await operation({
				kind: 'Matrix',
				media: targetMedia,
				edit: { kind: 'Inspect' }
			});
			return { value: result.value.snapshot, history: result.history };
		};
		const before = await inspect();
		const source = (await read()).value.cells.find((cell) => cell.id === a);
		const result = await operation({
			kind: 'Matrix',
			media: targetMedia,
			edit: { kind: 'TransferCell', cell: a, layer, column, action: mode }
		});
		const after = await inspect();
		const created = after.value.cells.find((cell) => cell.id === result.value.cell);
		assert.notEqual(created.id, a);
		assert.deepEqual(created.definition.settings, source.definition.settings);
		assert.equal(
			created.definition.application,
			mode === 'Reference' ? source.definition.application : created.id
		);
		const channels = (
			await operation({
				kind: 'InspectChannels',
				query: { media: created.media, start: 0, count: 32 }
			})
		).value.page;
		assert.equal(channels.total, 1);
		assert.equal(
			channels.channels[0].reference.channel === source.definition.source.channel,
			mode === 'Reference'
		);
		assert.equal(after.history.undo_len, before.history.undo_len + 1);
		assert((await intent({ kind: 'undo' })).success);
		assert.deepEqual((await inspect()).value, before.value);
		assert((await intent({ kind: 'redo' })).success);
		assert.deepEqual((await inspect()).value, after.value);
		checks.push({
			mode,
			cross_matrix: targetMedia !== reference.media,
			ownership: true,
			exact_history: true
		});
	}
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify({ result: 'pass', pid: host.app.pid, checks, native_pixels: false }, null, 2)
	);
	console.log(JSON.stringify({ result: 'pass', checks }));
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify({ error: String(error), checks }, null, 2)
	);
	throw error;
} finally {
	await host.close();
	await writeFile(path.join(directory, 'host.log'), host.log());
}
