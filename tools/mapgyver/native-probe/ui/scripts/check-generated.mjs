import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';

const generated = execFileSync(
	'cargo',
	['run', '--locked', '-q', '-p', 'mapgyver_native_probe', '--bin', 'generate_probe_bindings'],
	{
		cwd: new URL('../../../../..', import.meta.url),
		encoding: 'utf8',
	},
);
const checkedIn = readFileSync(new URL('../src/generated.ts', import.meta.url), 'utf8');
if (generated.replaceAll('\r\n', '\n') !== checkedIn.replaceAll('\r\n', '\n')) {
	throw new Error('Native probe DTO drift: regenerate with generate_probe_bindings.');
}
console.log('Native probe Rust/TypeScript DTOs match.');
