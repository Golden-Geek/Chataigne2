import { afterEach, describe, expect, it, vi } from 'vitest';
import { SurfaceMeasurements } from '../surface-measurements';

afterEach(() => vi.unstubAllGlobals());
function fixture() {
	const style = { fontSize: '16px', overflowX: 'hidden', overflowY: 'auto' };
	const readStyle = vi.fn(() => style);
	vi.stubGlobal('getComputedStyle', readStyle);
	const bounds = vi.fn(() => ({ left: 10, top: 20, width: 100, height: 80 }) as DOMRect);
	const element = {
		getBoundingClientRect: bounds,
		clientWidth: 100,
		clientHeight: 80
	} as unknown as HTMLElement;
	return { element, style, bounds, readStyle };
}
describe('page presentation measurement batches', () => {
	it('reads a shared ancestor once per synchronous batch and captures style values', async () => {
		const f = fixture(),
			measurements = new SurfaceMeasurements();
		const first = measurements.read(f.element);
		f.style.fontSize = '20px';
		for (let i = 0; i < 512; i++) expect(measurements.read(f.element)).toBe(first);
		expect(f.bounds).toHaveBeenCalledTimes(1);
		expect(f.readStyle).toHaveBeenCalledTimes(1);
		expect(first.fontSize).toBe(16);
		await Promise.resolve();
		expect(measurements.read(f.element).fontSize).toBe(20);
		expect(f.bounds).toHaveBeenCalledTimes(2);
	});
	it('keeps page ownership isolated and releases snapshots on explicit retirement', () => {
		const f = fixture(),
			first = new SurfaceMeasurements(),
			second = new SurfaceMeasurements();
		expect(first.read(f.element)).not.toBe(second.read(f.element));
		const previous = first.read(f.element);
		first.clear();
		expect(first.read(f.element)).not.toBe(previous);
		expect(f.bounds).toHaveBeenCalledTimes(3);
	});
});
