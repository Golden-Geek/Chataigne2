import assert from 'node:assert/strict';
import { test } from 'node:test';
import { warpEditGroup, type CornerLinkEdit } from '../warp-edit-group.ts';

const edit: CornerLinkEdit = {
	kind: 'LinkScreenCorners',
	source: { reference: { screen: 'screen', surface: 'a' }, corner: 0 },
	target: { reference: { screen: 'screen', surface: 'b' }, corner: 1 }
};

test('link acknowledgement precedes the existing Undo group close', async () => {
	const events: string[] = [];
	let release!: () => void;
	const ack = new Promise<void>((resolve) => {
		release = resolve;
	});
	const group = {
		active: true,
		async begin() {},
		async end() {
			events.push('end');
		}
	};
	const wrapped = warpEditGroup(
		group,
		() => true,
		() => edit,
		async (value) => {
			assert.equal(value, edit);
			events.push('link');
			await ack;
			events.push('accepted');
		}
	);
	const closing = wrapped.end();
	assert.deepEqual(events, ['link']);
	release();
	await closing;
	assert.deepEqual(events, ['link', 'accepted', 'end']);
});

test('invalidated contexts discard a pending link and failures still release the group', async () => {
	let links = 0,
		ends = 0,
		takes = 0;
	const group = {
		active: true,
		async begin() {},
		async end() {
			ends++;
		}
	};
	await warpEditGroup(
		group,
		() => false,
		() => {
			takes++;
			return edit;
		},
		async () => {
			links++;
		}
	).end();
	assert.equal(links, 0);
	assert.equal(takes, 1);
	assert.equal(ends, 1);
	await assert.rejects(
		warpEditGroup(
			group,
			() => true,
			() => edit,
			async () => {
				throw new Error('conflict');
			}
		).end(),
		/conflict/
	);
	assert.equal(ends, 2);
});
