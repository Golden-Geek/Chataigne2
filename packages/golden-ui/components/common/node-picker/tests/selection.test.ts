import { describe, expect, it } from 'vitest';
import { VersionedNodeMap } from '../../../../store/graph-index';
import type { UiNodeDto } from '../../../../types';
import { pickerCandidateAllowed, pickerProjectionAllowed } from '../selection';

const node = (id: number): UiNodeDto => ({ node_id: id, uuid: `node-${id}` }) as UiNodeDto;
const root = node(1),
	child = node(2),
	outside = node(3);
const graph = {
	nodesById: new VersionedNodeMap([
		[1, root],
		[2, child],
		[3, outside]
	]),
	parentById: new VersionedNodeMap([[2, 1]])
};

describe('picker selection authority', () => {
	it('rejects outside-root and replaced identities', () => {
		expect(pickerCandidateAllowed(graph, { rootNode: root }, null, child)).toBe(true);
		expect(pickerCandidateAllowed(graph, { rootNode: root }, null, outside)).toBe(false);
		expect(pickerCandidateAllowed(graph, { rootNode: root }, null, { ...child, uuid: 'old' })).toBe(
			false
		);
		expect(pickerCandidateAllowed(graph, { rootNode: { ...root, uuid: 'old' } }, null, child)).toBe(
			false
		);
	});
	it('applies global, view and row constraints to confirmation candidates', () => {
		expect(
			pickerCandidateAllowed(graph, { rootNode: root, nodeFilter: () => false }, null, child)
		).toBe(false);
		expect(
			pickerCandidateAllowed(
				graph,
				{ rootNode: root, nodeVisibilityFilter: () => false },
				null,
				child
			)
		).toBe(false);
		expect(
			pickerCandidateAllowed(
				graph,
				{ rootNode: root },
				{ id: 'view', label: 'View', nodeVisibilityFilter: () => false },
				child
			)
		).toBe(false);
		expect(
			pickerCandidateAllowed(
				graph,
				{ rootNode: root },
				{ id: 'view', label: 'View', nodeRowVisibilityFilter: () => false },
				child
			)
		).toBe(false);
	});
	it('terminates on malformed cyclic ancestry', () => {
		const cyclic = {
			...graph,
			parentById: new VersionedNodeMap([
				[2, 3],
				[3, 2]
			])
		};
		expect(pickerCandidateAllowed(cyclic, { rootNode: root }, null, child)).toBe(false);
	});
	it('requires an actual supported projection when the caller requires one', () => {
		const options = {
			rootNode: root,
			projectionOptions: () => ['vec2X', 'vec2Y'] as const,
			projectionRequired: () => true
		};
		const supported = { ...options, projectionOptions: () => [...options.projectionOptions()] };
		expect(pickerProjectionAllowed(supported, child, undefined)).toBe(false);
		expect(pickerProjectionAllowed(supported, child, 'vec2X')).toBe(true);
		expect(pickerProjectionAllowed(supported, child, 'floatToVec2XX')).toBe(false);
		expect(
			pickerProjectionAllowed({ ...supported, projectionRequired: () => false }, child, undefined)
		).toBe(true);
	});
});
