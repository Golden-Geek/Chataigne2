// Deterministic generated assets from the supplied source; no authored source image is modified.
import { execFileSync } from 'node:child_process';
import { copyFile, mkdir } from 'node:fs/promises';
import path from 'node:path';
const root = path.resolve(import.meta.dirname, '../..');
const generated = path.join(root, 'target/m119-icons');
execFileSync(
	process.execPath,
	[
		path.join(root, 'node_modules/@tauri-apps/cli/tauri.js'),
		'icon',
		path.join(root, 'mapgyver_icon.png'),
		'--output',
		generated
	],
	{ stdio: 'inherit' }
);
const icons = path.join(root, 'apps/mapgyver/icons');
await mkdir(icons, { recursive: true });
for (const file of [
	'icon.png',
	'icon.ico',
	'icon.icns',
	'32x32.png',
	'64x64.png',
	'128x128.png',
	'128x128@2x.png'
])
	await copyFile(path.join(generated, file), path.join(icons, file));
await mkdir(path.join(root, 'apps/mapgyver/assets'), { recursive: true });
await copyFile(
	path.join(generated, 'icon.png'),
	path.join(root, 'apps/mapgyver/assets/mapgyver-icon.png')
);
await copyFile(
	path.join(generated, '32x32.png'),
	path.join(root, 'apps/mapgyver/ui/static/favicon.png')
);
await copyFile(
	path.join(generated, '128x128.png'),
	path.join(root, 'apps/mapgyver/ui/static/mapgyver-icon.png')
);
