import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createCellTransfers, readCellDrag } from '../cell-transfer.ts';
import type { MatrixEdit } from '../../media/generated/MatrixEdit';
import type { MediaEditResult } from '../../media/generated/MediaEditResult';

function fixture() {
	let session = { status: 'connected', snapshotGeneration: 1, loading: { visible: false } };
	let target = { project: 'project', media: 'matrix' },
		busy = false,
		adopt = true;
	const edits: MatrixEdit[] = [],
		selections: string[] = [],
		addresses: string[] = [];
	let result: Promise<MediaEditResult | undefined> = Promise.resolve({ kind: 'Changed' });
	const owner = createCellTransfers({
		session: () => session,
		target: () => target,
		busy: () => busy,
		edit: async (value) => {
			edits.push(value);
			return result;
		},
		captureSelection: () => (uuid) => {
			if (adopt) selections.push(uuid);
			return adopt;
		},
		selectAddress: (row, column) => addresses.push(`${row}:${column}`)
	});
	return {
		owner,
		edits,
		selections,
		addresses,
		get session() {
			return session;
		},
		busy: (value: boolean) => {
			busy = value;
		},
		target: (value: typeof target) => {
			target = value;
		},
		replace: () => {
			session = { ...session };
		},
		adopt: (value: boolean) => {
			adopt = value;
		},
		result: (value: Promise<MediaEditResult | undefined>) => {
			result = value;
		}
	};
}

test('Cell drags carry bounded identities only and reject malformed payloads', () => {
	const f = fixture(),
		source = f.owner.source('cell')!;
	assert.deepEqual(
		readCellDrag(JSON.stringify({ ...source, authored_settings: { opacity: 0 } })),
		source
	);
	for (const value of [
		'',
		'{',
		'null',
		'[]',
		JSON.stringify({ ...source, generation: -1 }),
		JSON.stringify({ ...source, cell: '' }),
		'x'.repeat(2049)
	])
		assert.equal(readCellDrag(value), null);
	f.busy(true);
	assert.equal(f.owner.source('cell'), null);
});

test('drop forwards one backend policy intent; Alt actions wait for a real choice', async () => {
	const f = fixture(),
		source = f.owner.source('cell')!;
	const menu = f.owner.menu(source, 'row', 'column');
	assert.deepEqual(
		menu.map((value) => value.label),
		['Move', 'Copy', 'Reference']
	);
	assert.equal(f.edits.length, 0);
	await f.owner.run(source, 'row', 'column', 'Drop');
	assert.deepEqual(f.edits, [
		{ kind: 'TransferCell', cell: 'cell', layer: 'row', column: 'column', action: 'Drop' }
	]);
	for (const item of menu) await item.action?.({} as MouseEvent, item);
	assert.deepEqual(
		f.edits.map((value) => value.kind === 'TransferCell' && value.action),
		['Drop', 'Move', 'Copy', 'Reference']
	);
});

test('menu, drag and clipboard fence project, generation, session and displayed Matrix ownership', async () => {
	for (const change of [
		'project',
		'generation',
		'session',
		'matrix',
		'disconnect',
		'loading'
	] as const) {
		const f = fixture(),
			source = f.owner.source('cell')!,
			menu = f.owner.menu(source, 'row', 'column');
		assert(f.owner.copy('cell'));
		if (change === 'project') f.target({ project: 'other', media: 'matrix' });
		if (change === 'matrix') f.target({ project: 'project', media: 'other' });
		if (change === 'generation') f.session.snapshotGeneration++;
		if (change === 'session') f.replace();
		if (change === 'disconnect') f.session.status = 'disconnected';
		if (change === 'loading') f.session.loading.visible = true;
		await menu[0].action?.({} as MouseEvent, menu[0]);
		assert.equal(f.edits.length, 0, change);
		if (change !== 'matrix') {
			await f.owner.paste('row', 'column');
			assert.equal(f.edits.length, 0, change);
		}
	}
});

test('clipboard supports cross-Matrix identity transfer, retains rejected cuts and clears only successful moves', async () => {
	const f = fixture();
	assert(f.owner.copy('cell', true));
	f.target({ project: 'project', media: 'other' });
	f.result(Promise.resolve(undefined));
	assert.equal(await f.owner.paste('row', 'column'), false);
	f.result(Promise.resolve({ kind: 'Changed' }));
	assert.equal(await f.owner.paste('row', 'column'), true);
	assert.equal(await f.owner.paste('row', 'column'), false);
	assert.equal(f.edits.length, 2);
	assert(f.edits.every((edit) => edit.kind === 'TransferCell' && edit.action === 'Move'));
	assert(f.owner.copy('new-cell'));
	f.result(
		Promise.resolve({ kind: 'MatrixCellPrepared', cell: 'committed-cell', application: 'app' })
	);
	assert(await f.owner.paste('row', 'column', true));
	assert.equal(f.selections.at(-1), 'committed-cell');
});

test('late replies and changed selection cannot steal a new target or adopt a rejected identity', async () => {
	let finish!: (value: MediaEditResult) => void;
	const f = fixture(),
		source = f.owner.source('cell')!;
	f.result(
		new Promise((resolve) => {
			finish = resolve;
		})
	);
	const pending = f.owner.run(source, 'row', 'column', 'Copy');
	f.target({ project: 'project', media: 'other' });
	finish({ kind: 'MatrixCellPrepared', cell: 'late', application: 'app' });
	assert.equal(await pending, false);
	assert.deepEqual(f.selections, []);
	assert.deepEqual(f.addresses, []);
	f.adopt(false);
	f.result(Promise.resolve({ kind: 'Changed' }));
	await f.owner.run(source, 'row', 'column', 'Move');
	assert.deepEqual(f.addresses, []);
});
