import { describe, expect, it } from 'vitest';
import { formatTimeValue, parseTimeValue } from '../time-value';

describe('numeric Parameter clock units', () => {
	it('keeps the seconds contract and supports declared millisecond storage', () => {
		expect(formatTimeValue(1.25)).toBe('00:00:01.250');
		expect(formatTimeValue(1250, 'ms')).toBe('00:00:01.250');
		expect(parseTimeValue('00:00:01.250')).toBe(1.25);
		expect(parseTimeValue('00:00:01.250', 'ms')).toBe(1250);
		for (const value of [0, 1, 250, 5000, 600000, 4294967295, -1250])
			expect(parseTimeValue(formatTimeValue(value, 'ms'), 'ms')).toBe(value);
	});
	it('does not introduce fractional raw milliseconds when parsing a clock', () => {
		for (let value = 0; value < 1000; value++) {
			const clock = `00:00:00.${value.toString().padStart(3, '0')}`;
			expect(parseTimeValue(clock, 'ms')).toBe(value);
		}
		expect(parseTimeValue('1:02.345', 'ms')).toBe(62345);
		expect(parseTimeValue('-2.4', 'ms')).toBe(-2400);
	});
	it('rejects malformed clocks and values outside exact millisecond precision', () => {
		for (const value of [
			'',
			'1:60',
			'1:60:00',
			'1:02:60',
			'1::2',
			'1.2.3',
			'NaN',
			'1e3',
			'999999999999999999999'
		])
			expect(parseTimeValue(value, 'ms')).toBeNull();
	});
});
