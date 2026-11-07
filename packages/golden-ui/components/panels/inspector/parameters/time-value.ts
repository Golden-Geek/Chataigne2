/** Clock presentation for numeric Parameters; hints declare the stored unit, never change it. */
export function timeFractionDigits(step?: number, unit?: string | null, value?: number): number {
	const seconds = step === undefined ? 0.001 : step / (unit === 'ms' ? 1000 : 1);
	const actual = value === undefined ? 0 : value / (unit === 'ms' ? 1000 : 1);
	for (let digits = 3; digits < 12; digits++) {
		const scale = 10 ** digits;
		if (
			Math.abs(seconds * scale - Math.round(seconds * scale)) < 1e-8 &&
			Math.abs(actual * scale - Math.round(actual * scale)) < 1e-8
		)
			return digits;
	}
	return 12;
}

export function formatTimeValue(value: number, unit?: string | null, fractionDigits = 3): string {
	if (!Number.isFinite(value)) return '00:00:00.000';
	const sign = value < 0 ? '-' : '';
	const digits = Math.max(3, Math.min(12, Math.round(fractionDigits)));
	// Split in the authored unit before clock conversion: scaling a large float into
	// fractional ticks loses precision even when its authored decimal is representable.
	const [whole, sub = ''] = Math.abs(value)
		.toFixed(unit === 'ms' ? digits - 3 : digits)
		.split('.');
	let remaining = unit === 'ms' ? Math.floor(Number(whole) / 1000) : Number(whole);
	const fraction =
		unit === 'ms' ? `${(Number(whole) % 1000).toString().padStart(3, '0')}${sub}` : sub;
	const hours = Math.floor(remaining / 3600);
	remaining -= hours * 3600;
	const minutes = Math.floor(remaining / 60);
	const seconds = remaining - minutes * 60;
	return `${sign}${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')}.${fraction}`;
}

export function parseTimeValue(text: string, unit?: string | null): number | null {
	const raw = text.trim();
	if (!raw) return null;
	const sign = raw.startsWith('-') ? -1 : 1;
	const unsigned = sign < 0 ? raw.slice(1) : raw;
	const [clock = '', fraction = ''] = unsigned.split('.');
	if (unsigned.split('.').length > 2 || !/^\d*$/.test(fraction)) return null;
	const segments = clock.split(':');
	if (segments.length > 3 || segments.some((segment) => !/^\d+$/.test(segment))) return null;
	let hours = 0,
		minutes = 0,
		seconds = 0;
	if (segments.length === 3) [hours, minutes, seconds] = segments.map(Number);
	else if (segments.length === 2) [minutes, seconds] = segments.map(Number);
	else seconds = Number(segments[0]);
	if ((segments.length === 3 && minutes >= 60) || (segments.length >= 2 && seconds >= 60))
		return null;
	const wholeMilliseconds = (hours * 3600 + minutes * 60 + seconds) * 1000;
	if (!Number.isSafeInteger(wholeMilliseconds) || fraction.length > 12) return null;
	const milliseconds = wholeMilliseconds + Number((fraction + '000').slice(0, 3));
	if (!Number.isSafeInteger(milliseconds)) return null;
	// Parse once in the authored unit rather than adding independently rounded floats.
	const authored =
		unit === 'ms'
			? `${milliseconds}.${fraction.slice(3)}`
			: `${wholeMilliseconds / 1000}.${fraction}`;
	return sign * Number(authored);
}
