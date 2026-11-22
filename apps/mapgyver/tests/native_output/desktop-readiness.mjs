import { execFile } from 'node:child_process';
import { mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { promisify } from 'node:util';
import { openInitialScreenDesktop } from '../headless/initial-screen-host.mjs';

const directory = await mkdtemp(join(tmpdir(), 'mapgyver-desktop-readiness-'));
const host = await openInitialScreenDesktop(directory);
try {
	const { stdout } = await promisify(execFile)(
		process.execPath,
		[
			'tools/product-gate/hooks/strict-ui-readiness.mjs',
			'--repository-root',
			process.cwd(),
			'--url',
			host.base + '/',
			'--screenshot',
			join(directory, 'ready.png'),
			'--timeout-ms',
			'70000'
		],
		{ timeout: 75000 }
	);
	console.log(stdout);
} catch (error) {
	console.error(host.log());
	throw error;
} finally {
	await host.close();
}
console.log('Native desktop host exited normally');
