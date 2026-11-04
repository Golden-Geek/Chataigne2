/** Clock presentation for numeric Parameters; hints declare the stored unit, never change it. */
export function formatTimeValue(value: number, unit?: string | null): string {
	if (!Number.isFinite(value)) return '00:00:00.000';
	const sign = value < 0 ? '-' : '';
	let remaining = Math.round(Math.abs(value) * (unit === 'ms' ? 1 : 1000));
	const hours = Math.floor(remaining / 3_600_000);
	remaining -= hours * 3_600_000;
	const minutes = Math.floor(remaining / 60_000);
	remaining -= minutes * 60_000;
	const seconds = Math.floor(remaining / 1000);
	const milliseconds = remaining - seconds * 1000;
	return `${sign}${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')}.${milliseconds.toString().padStart(3, '0')}`;
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
	const milliseconds =
		(hours * 3600 + minutes * 60 + seconds) * 1000 + Number((fraction + '000').slice(0, 3));
	if (!Number.isSafeInteger(milliseconds)) return null;
	return sign * (unit === 'ms' ? milliseconds : milliseconds / 1000);
}
