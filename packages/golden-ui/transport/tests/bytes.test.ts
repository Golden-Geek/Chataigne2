import { describe, expect, it } from 'vitest';
import { fromRustParamValue, toRustParamValue } from '../codecs/values';

describe('binary parameter transport', () => {
	it.each([[], [0, 255], [1, 2, 3], [0, 1, 2, 3], Array(512).fill(255)])(
		'preserves byte arrays across generated wire and UI representations (%j)',
		(...bytes: number[]) => {
			const wire = { Bytes: bytes };
			const value = fromRustParamValue(wire);
			expect(value).toEqual({ kind: 'bytes', value: bytes });
			expect(toRustParamValue(value)).toEqual(wire);
		}
	);
});
