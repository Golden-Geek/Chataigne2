// M07 ordinary-host parity and a persistent, initially unlaunched real-editor review scene.
import assert from 'node:assert/strict';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import path from 'node:path';
import { clearMatrixScene } from '../browser/matrix-scene.mjs';

export async function verifyMatrix({ post, intent, edit }) {
	const create = async (media_kind, label) =>
		(await edit({ kind: 'CreateMedia', media_kind, label })).reference;
	const sources = [];
	for (const [label, rgba] of [
		['Matrix Red', [1, 0, 0, 1]],
		['Matrix Green', [0, 1, 0, 1]],
		['Matrix Blue', [0, 0, 1, 1]]
	]) {
		const reference = await create('SolidColor', label);
		await edit({
			kind: 'ConfigureChannel',
			reference,
			source: { kind: 'SolidColor', rgba }
		});
		await edit({
			kind: 'ConfigureMediaResolution',
			media: reference.media,
			size: { kind: 'Fixed', extent: { width: 320, height: 240 } }
		});
		sources.push(reference);
	}
	const root = await create('Matrix', 'Review Matrix');
	await clearMatrixScene(edit, root.media);
	await edit({
		kind: 'ConfigureMediaResolution',
		media: root.media,
		size: { kind: 'Fixed', extent: { width: 320, height: 240 } }
	});
	const matrix = (value) => edit({ kind: 'Matrix', media: root.media, edit: value });
	const inspect = async () => (await matrix({ kind: 'Inspect' })).snapshot;
	const playback = async () => (await matrix({ kind: 'InspectPlayback' })).snapshot;
	const viewport = { row_start: 0, row_count: 32, axis_start: 0, axis_count: 16 };
	const window = async () => (await matrix({ kind: 'InspectViewport', viewport })).snapshot;
	await matrix({
		kind: 'Configure',
		settings: {
			empty_cell: 'Transparent'
		}
	});
	const {
		items: [group]
	} = await matrix({ kind: 'AddGroup', parent: null, label: 'Review Group' });
	const { items: layers } = await matrix({ kind: 'AddLayers', parent: group.id, count: 2 });
	for (const layer of layers)
		await matrix({
			kind: 'ConfigureLayer',
			layer: layer.id,
			settings: { position: [0, 0], size: { width: 320, height: 240 }, duration_ms: 500 }
		});
	const { columns } = await matrix({ kind: 'AddColumns', count: 3 });
	for (const [index, label] of ['Lower', 'Upper'].entries())
		await matrix({ kind: 'RenameItem', item: layers[index], label });
	for (const [index, label] of ['Red + Green', 'Green + Red', 'Blue'].entries())
		await matrix({ kind: 'RenameColumn', column: columns[index], label });
	await matrix({
		kind: 'ConfigureItem',
		item: layers[1],
		settings: { opacity: 0.5, blend: 'Normal', enabled: true }
	});
	await matrix({
		kind: 'ConfigureItem',
		item: group,
		settings: { opacity: 0.8, blend: 'Normal', enabled: true }
	});
	await matrix({
		kind: 'SetCells',
		assignments: [
			{ layer: layers[0].id, column: columns[0], source: sources[0] },
			{ layer: layers[1].id, column: columns[0], source: sources[1] },
			{ layer: layers[0].id, column: columns[1], source: sources[1] },
			{ layer: layers[1].id, column: columns[1], source: sources[0] },
			{ layer: layers[0].id, column: columns[2], source: sources[2] }
		]
	});
	const review = await inspect();
	assert.equal((await playback()).desired_column, null);
	const initialWindow = await window();
	assert.equal(initialWindow.rows.length, 3);
	assert.equal(initialWindow.cells.length, 5);
	assert.equal(
		(await playback()).desired_column,
		null,
		'inspecting/selecting a viewport is not launch'
	);
	const directory = path.resolve(
		process.env.MAPGYVER_MATRIX_REVIEW_DIRECTORY ?? 'target/m07-grid-review'
	);
	await mkdir(directory, { recursive: true });
	const project = path.join(directory, 'MatrixReview.mapgyver');
	assert.equal((await post('/api/ui/project-save', { path: project })).ok, true);
	for (const mode of ['Cut', 'Configured']) {
		const reply = await matrix({ kind: 'LaunchColumn', column: columns[1], mode });
		assert.equal(reply.kind, 'MatrixLaunched');
		assert.equal(reply.snapshot.desired_column, columns[1]);
		assert.equal((await playback()).desired_column, columns[1]);
		assert.deepEqual(
			await inspect(),
			review,
			'launch must not alter authored rows, columns or cells'
		);
	}
	await matrix({ kind: 'ClearPlayback' });
	assert.equal((await playback()).desired_column, null);
	const beforeAssign = await inspect();
	await matrix({
		kind: 'SetCells',
		assignments: [{ layer: layers[0].id, column: columns[0], source: sources[2] }]
	});
	assert.equal(
		(await inspect()).cells.find(
			(c) => c.layer === layers[0].id && c.definition.column === columns[0]
		).definition.source.channel,
		sources[2].channel
	);
	assert.equal((await playback()).desired_column, null, 'source assignment never launches');
	assert((await intent({ kind: 'undo' })).success);
	assert.deepEqual(await inspect(), beforeAssign);
	assert((await intent({ kind: 'redo' })).success);
	assert.notDeepEqual(await inspect(), beforeAssign);
	assert((await intent({ kind: 'undo' })).success);
	for (const operation of [
		{ kind: 'MoveItem', item: layers[0], parent: null, after: group },
		{ kind: 'ReorderColumn', column: columns[0], after: columns[2] },
		{ kind: 'DuplicateItem', item: group },
		{ kind: 'DuplicateColumn', column: columns[0] },
		{ kind: 'RemoveItem', item: layers[1] },
		{ kind: 'RemoveColumn', column: columns[1] }
	]) {
		const before = await inspect();
		await matrix(operation);
		const after = await inspect();
		assert.notDeepEqual(after, before);
		assert((await intent({ kind: 'undo' })).success);
		assert.deepEqual(await inspect(), before);
		assert((await intent({ kind: 'redo' })).success);
		assert.deepEqual(await inspect(), after);
		assert((await intent({ kind: 'undo' })).success);
	}
	const rejected = await intent({
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.media',
		operation: 'edit',
		payload: {
			kind: 'Matrix',
			media: root.media,
			edit: {
				kind: 'SetCells',
				assignments: [{ layer: layers[0].id, column: columns[0], source: root }]
			}
		}
	});
	assert.equal(rejected.success, false);
	assert.deepEqual(await inspect(), review);
	await edit({ kind: 'RemoveMedia', media: sources[0].media });
	const redAliases = new Set(
		review.cells
			.filter((cell) => cell.definition.source?.channel === sources[0].channel)
			.map((cell) => cell.media)
	);
	const missing = (await window()).cells;
	assert.equal(missing.filter((entry) => redAliases.has(entry.cell.media)).length, 2);
	assert(missing.every((entry) => entry.source_missing === redAliases.has(entry.cell.media)));
	assert((await intent({ kind: 'undo' })).success);
	assert.deepEqual(await inspect(), review);
	assert.equal((await post('/api/ui/project-new', {})).ok, true);
	await post('/api/ui/project-load', { path: project });
	assert.deepEqual(await inspect(), review);
	assert.equal((await playback()).desired_column, null);
	const loaded = await window();
	assert.notEqual(loaded.project, initialWindow.project);
	assert.deepEqual(loaded.rows, initialWindow.rows);
	assert.deepEqual(loaded.cells, initialWindow.cells);
	const report = {
		result: 'pass',
		scope:
			'ordinary headless Matrix caller parity; real grid interaction and rendering remain unverified',
		project,
		project_sha256: createHash('sha256')
			.update(await readFile(project))
			.digest('hex'),
		root,
		sources,
		group,
		layers,
		columns,
		rows: 3,
		populated_cells: 5,
		initially_unlaunched: true,
		history_replay_operations: 7,
		original_scene_restored: true
	};
	await writeFile(path.join(directory, 'review.json'), JSON.stringify(report, null, 2));
	console.log(JSON.stringify(report));
}
