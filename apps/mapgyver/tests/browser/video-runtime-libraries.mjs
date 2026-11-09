// Read only modules of the owned app; verify executable-local pinned runtime deployment.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import path from 'node:path';

export async function inspectLocalVideoLibraries(
	pid,
	binary,
	pinPath = 'tools/mapgyver/ffmpeg/windows-x64.json'
) {
	assert(Number.isSafeInteger(pid) && pid > 0);
	const pin = JSON.parse(await readFile(pinPath, 'utf8'));
	const modules = JSON.parse(
		execFileSync(
			'powershell',
			[
				'-NoProfile',
				'-NonInteractive',
				'-Command',
				`@((Get-Process -Id ${pid}).Modules | Where-Object { $_.ModuleName -match '^(av(codec|format|util)|sw(resample|scale))-[0-9]+\\.dll$' } | Select-Object ModuleName,FileName) | ConvertTo-Json -Compress`
			],
			{ encoding: 'utf8', windowsHide: true, timeout: 5000 }
		)
	);
	assert.equal(modules.length, pin.runtime.length, 'All pinned FFmpeg libraries must be loaded');
	const directory = path.dirname(path.resolve(binary)).toLowerCase();
	const inspected = [];
	for (const library of pin.runtime) {
		const loaded = modules.find((module) => module.ModuleName.toLowerCase() === library.name);
		assert(loaded, `Missing loaded ${library.name}`);
		assert.equal(path.dirname(path.resolve(loaded.FileName)).toLowerCase(), directory);
		const bytes = await readFile(loaded.FileName);
		assert.equal(bytes.length, library.bytes);
		const sha256 = createHash('sha256').update(bytes).digest('hex');
		assert.equal(sha256, library.sha256);
		inspected.push({ name: library.name, path: loaded.FileName, bytes: bytes.length, sha256 });
	}
	return inspected;
}
