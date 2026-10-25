import { describe, expect, it } from 'vitest';
import { FlowLengths, flowSegments } from '../flow-layout';
import type { NonFreeSurfaceFlowRenderItem } from '../contracts';
import { node } from './fixtures';

describe('bounded flow extents', () => {
	it('sums each serialized slot length with internal gaps and automatic slots', () => {
		const lengths = new FlowLengths([
			{ key: 'a', size: { value: 10, unit: 'px' } },
			{ key: 'b', size: null },
			{ key: 'c', size: { value: 1.5, unit: 'rem' } }
		]);
		const expression = lengths.expression(0, 3, 'max(10%, 0px)', { value: 0.5, unit: 'em' });
		expect(expression).toContain('10px');
		expect(expression).toContain('1.5rem');
		expect(expression).toContain('max(10%, 0px) * 1');
		expect(expression).toContain('0.5em * 2');
		expect(expression.startsWith('calc(')).toBe(true);
	});
	it('preserves per-slot rounding over a long omitted range', () => {
		const lengths = new FlowLengths(
			Array.from({ length: 10000 }, (_, index) => ({
				key: String(index),
				size: { value: 1.234567, unit: 'px' as const }
			}))
		);
		expect(lengths.expression(0, 10000, '0px', { value: 0, unit: 'rem' })).toContain('12346px');
		expect(lengths.index('9999')).toBe(9999);
	});
	it('keeps mounted item identities and replaces only omitted ranges with bounded spacers', () => {
		const items: NonFreeSurfaceFlowRenderItem[] = Array.from({ length: 100 }, (_, id) => ({
			kind: 'child',
			key: String(id),
			child: node(id, 'dashboard_generic_widget')
		}));
		const lengths = new FlowLengths(
			items.map((item) => ({ key: item.key, size: { value: 1, unit: 'rem' } }))
		);
		const segments = flowSegments(
			[items[30], items[80]],
			lengths,
			(start, end) => `${start}:${end}`
		);
		expect(segments.map((item) => (item.kind === 'spacer' ? item.style : item.item.key))).toEqual([
			'0:30',
			'30',
			'31:80',
			'80',
			'81:100'
		]);
		expect(segments[1].kind === 'item' && segments[1].item).toBe(items[30]);
		expect(
			flowSegments(items, null, () => {
				throw Error('No omitted ranges');
			}).length
		).toBe(100);
	});
});
