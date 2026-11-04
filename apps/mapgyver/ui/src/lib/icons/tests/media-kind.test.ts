import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { UiNodeDto } from 'golden_ui';
import { mediaIconKind } from '../media-kind.ts';

test('media icon classification reads the real immutable Parameter with bounded work', () => {
	const root = {
		node_type: 'mapgyver.media.v8',
		children: [1, ...Array.from({ length: 10000 }, (_, i) => i + 3)]
	} as UiNodeDto;
	const nodes = new Map<number, UiNodeDto>([
		[1, { decl_id: 'parameters', children: [2] } as UiNodeDto],
		[
			2,
			{
				decl_id: 'parameters/media_kind',
				data: { kind: 'parameter', param: { value: { kind: 'enum', value: 'Matrix' } } }
			} as UiNodeDto
		]
	]);
	let reads = 0;
	assert.equal(
		mediaIconKind(root, (id) => {
			reads++;
			return nodes.get(id);
		}),
		'Matrix'
	);
	assert.equal(reads, 4);
	assert.equal(
		mediaIconKind({ node_type: 'mapgyver.media.reference.v3' } as UiNodeDto, () => undefined),
		'Reference'
	);
	assert.equal(
		mediaIconKind(root, () => undefined),
		null
	);
});
