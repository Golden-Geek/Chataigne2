import { describe, expect, it } from 'vitest';
import type { GraphNode, GraphNodePosition } from 'golden_graph_ui';
import {
	DEFAULT_STATE_HEIGHT_REM,
	DEFAULT_STATE_WIDTH_REM,
	nearestFreeStatePosition
} from '../state-placement';

const stateNode = (id: number, position: GraphNodePosition, width = 13, height = 8): GraphNode => ({
	id: String(id),
	label: `State ${id}`,
	position,
	size: { width, height },
	inputs: [],
	outputs: []
});

const width = (node: GraphNode): number => node.size?.width ?? DEFAULT_STATE_WIDTH_REM;
const height = (node: GraphNode): number => node.size?.height ?? DEFAULT_STATE_HEIGHT_REM;

const isClearOf = (position: GraphNodePosition, node: GraphNode): boolean =>
	position.x >= node.position.x + width(node) + 1 ||
	position.x + DEFAULT_STATE_WIDTH_REM + 1 <= node.position.x ||
	position.y >= node.position.y + height(node) + 1 ||
	position.y + DEFAULT_STATE_HEIGHT_REM + 1 <= node.position.y;

describe('state-machine canvas placement hint', () => {
	it('centers the default state on a half-rem grid in an empty graph', () => {
		expect(nearestFreeStatePosition({ x: 0, y: 0 }, [], width, height)).toEqual({
			x: -6.5,
			y: -4
		});
		expect(nearestFreeStatePosition({ x: 1.2, y: 2.7 }, [], width, height)).toEqual({
			x: -5.5,
			y: -1.5
		});
	});

	it('finds a deterministic clear position when the preferred center is occupied', () => {
		const occupied = stateNode(1, { x: -6.5, y: -4 });
		const center = { x: 0, y: 0 };
		const position = nearestFreeStatePosition(center, [occupied], width, height);
		expect(position).not.toEqual({ x: -6.5, y: -4 });
		expect(isClearOf(position, occupied)).toBe(true);
		expect(nearestFreeStatePosition(center, [occupied], width, height)).toEqual(position);
	});

	it('accounts for enlarged existing states', () => {
		const occupied = stateNode(1, { x: 0, y: 0 }, 40, 20);
		const position = nearestFreeStatePosition({ x: 30, y: 12 }, [occupied], width, height);
		expect(isClearOf(position, occupied)).toBe(true);
	});

	it('uses the spatial index with 10k states without overlapping an occupied region', () => {
		const states = Array.from({ length: 10_000 }, (_, index) =>
			stateNode(index, { x: index * 32, y: 0 })
		);
		const position = nearestFreeStatePosition({ x: 6.5, y: 4 }, states, width, height);
		expect(position).not.toEqual({ x: 0, y: 0 });
		expect(states.every((state) => isClearOf(position, state))).toBe(true);
	});
});
