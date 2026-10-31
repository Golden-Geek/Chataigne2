import type { ApplicationDefinition } from '../media/generated/ApplicationDefinition';
import type { ApplicationReadiness } from '../media/generated/ApplicationReadiness';
import type { ApplicationWindow } from '../media/generated/ApplicationWindow';

function text(data: FormData, key: string) {
	const value = data.get(key);
	if (typeof value !== 'string') throw new Error(`Enter ${key}`);
	return value;
}
function number(data: FormData, key: string) {
	const value = text(data, key);
	if (!value.trim() || !Number.isFinite(Number(value)))
		throw new Error(`Enter a number for ${key}`);
	return Number(value);
}
/** Decode explicit form inputs; backend validation and defaults remain authoritative. */
export function definitionFromForm(
	data: FormData,
	base: ApplicationDefinition,
	readiness: ApplicationReadiness['kind'],
	sender: string
): ApplicationDefinition {
	const names = data.getAll('environment-name'),
		values = data.getAll('environment-value');
	if (names.length !== values.length)
		throw new Error('Each environment entry needs a name and value');
	const evidence = readiness === 'Spout' ? text(data, 'readiness-evidence') : null;
	if (evidence !== null && evidence !== 'sender' && evidence !== 'image')
		throw new Error('Choose sender or first-image readiness');
	return {
		...base,
		executable: text(data, 'executable') || null,
		directory: text(data, 'directory') || null,
		arguments: data.getAll('argument').map(String),
		environment: data.has('inherit-environment')
			? null
			: names.map((name, index) => [String(name), String(values[index])]),
		window: text(data, 'window') as ApplicationWindow,
		readiness:
			readiness === 'ProcessStarted'
				? { kind: 'ProcessStarted' }
				: {
						kind: 'Spout',
						sender,
						first_frame: evidence === 'image',
						timeout_ms: number(data, 'readiness-timeout')
					},
		stop: {
			graceful_ms: number(data, 'graceful-stop'),
			retirement_ms: number(data, 'retirement-timeout')
		},
		restart: {
			enabled: data.has('restart'),
			initial_ms: number(data, 'restart-initial'),
			maximum_ms: number(data, 'restart-maximum')
		}
	};
}
