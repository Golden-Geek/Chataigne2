import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, readdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const workspaceRoot = resolve(packageRoot, '..', '..');
const expectedRoot = join(packageRoot, 'generated');
const temporaryRoot = mkdtempSync(join(tmpdir(), 'golden-audio-ui-codegen-'));

const generatedFiles = (root, directory = root) =>
	readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
		const entryPath = join(directory, entry.name);
		return entry.isDirectory() ? generatedFiles(root, entryPath) : [relative(root, entryPath)];
	});

try {
	const generation = spawnSync(
		'cargo',
		[
			'run',
			'-p',
			'golden_audio',
			'--features',
			'codegen',
			'--bin',
			'generate_golden_audio_contract',
			'--',
			temporaryRoot
		],
		{ cwd: workspaceRoot, encoding: 'utf8' }
	);
	if (generation.status !== 0) {
		process.stderr.write(generation.stdout);
		process.stderr.write(generation.stderr);
		process.exitCode = generation.status ?? 1;
	} else {
		const expectedFiles = generatedFiles(expectedRoot).sort();
		const actualFiles = generatedFiles(temporaryRoot).sort();
		const drift = new Set([...expectedFiles, ...actualFiles]);
		const changed = [...drift].filter((file) => {
			if (!expectedFiles.includes(file) || !actualFiles.includes(file)) return true;
			return (
				readFileSync(join(expectedRoot, file), 'utf8') !==
				readFileSync(join(temporaryRoot, file), 'utf8')
			);
		});
		if (changed.length > 0) {
			process.stderr.write(
				`golden_audio_ui generated bindings are stale:\n${changed
					.map((file) => `  ${file}`)
					.join('\n')}\nRun npm run codegen --workspace golden_audio_ui.\n`
			);
			process.exitCode = 1;
		}
	}
} finally {
	rmSync(temporaryRoot, { recursive: true, force: true });
}
