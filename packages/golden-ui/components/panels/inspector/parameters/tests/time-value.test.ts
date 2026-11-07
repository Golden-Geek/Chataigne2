import { describe, expect, it } from 'vitest';
import { formatTimeValue, parseTimeValue, timeFractionDigits } from '../time-value';

describe('numeric Parameter clock units', () => {
	it('retains fractional raw milliseconds and declared seconds precision', () => {
		for (const [value, unit, step] of [
			[1234.25, 'ms', 0.25],
			[1.2345, 's', 0.0001]
		] as const) {
			const clock = formatTimeValue(value, unit, timeFractionDigits(step, unit));
			expect(parseTimeValue(clock, unit)).toBe(value);
		}
		expect(formatTimeValue(1234.25, 'ms', 5)).toBe('00:00:01.23425');
		expect(parseTimeValue('00:00:00.00025', 'ms')).toBe(0.25);
		const fine = formatTimeValue(
			1.23456789,
			'ms',
			timeFractionDigits(0.00000001, 'ms', 1.23456789)
		);
		expect(parseTimeValue(fine, 'ms')).toBe(1.23456789);
		expect(formatTimeValue(1.234567, 's', timeFractionDigits(undefined, 's', 1.234567))).toBe(
			'00:00:01.234567'
		);
	});
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
