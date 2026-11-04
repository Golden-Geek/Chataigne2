import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdir, writeFile, access } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { verifyRuntimeFooter, verifyRuntimeFooterRecovery } from './runtime-footer.mjs';

/** Actual native catalog and editor projections; no physical display is enabled by this check. */
export async function verifyScreenIdentity(options) {
	const { page, post, edit } = options;
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	await page.locator('.gc-dv-tab[data-panel-type="screenManager"]').waitFor();
	const identityArtifacts = path.resolve('target/m119-window-identity');
	await mkdir(identityArtifacts, { recursive: true });
	const nativeIdentity = JSON.parse(
		execFileSync(
			'powershell',
			[
				'-NoLogo',
				'-NoProfile',
				'-NonInteractive',
				'-ExecutionPolicy',
				'Bypass',
				'-File',
				path.resolve('apps/mapgyver/tests/native_output/identity.ps1'),
				'-AppProcessId',
				String(options.nativePid),
				'-ArtifactDirectory',
				identityArtifacts
			],
			{ encoding: 'utf8', timeout: 15000, windowsHide: true }
		)
	);
	assert.equal(nativeIdentity.process, options.nativePid);
	assert.equal(nativeIdentity.icons.length, 2);
	assert.equal(nativeIdentity.icons.find((icon) => icon.type === 0)?.source, 'window');
	console.log(JSON.stringify({ nativeWindowIdentity: nativeIdentity }));
	await post('/api/ui/project-new', {});
	await page.setViewportSize({ width: 1200, height: 850 });
	const screens = async (payload) => {
		const result = await post('/api/ui/intent', {
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.screens',
			operation: 'edit',
			payload
		});
		assert(result.success, JSON.stringify(result));
		return result.result.value;
	};
	const displays = await page.evaluate(async () =>
		JSON.parse(
			await window.__TAURI_INTERNALS__.invoke('native_presentation_control', {
				request: JSON.stringify({ kind: 'mapgyverDisplays', refresh: true })
			})
		)
	);
	assert(displays.displays.length > 0, JSON.stringify(displays));
	const display = displays.displays[0];
	const created = await screens({ kind: 'CreateScreen', label: 'Identity screen' });
	await screens({
		kind: 'ConfigureScreen',
		screen: created.screen,
		definition: { enabled: false, target: { kind: 'Fullscreen', display: display.id } }
	});
	const row = (await screens({ kind: 'InspectRow', node: created.screen })).snapshot;
	assert(
		row && !row.detail.includes(display.id) && !row.detail.includes('\\'),
		JSON.stringify(row)
	);
	assert.match(row.detail, /^Fullscreen · /);
	assert(!row.detail.includes('(missing)'), 'a connected disabled display is present');
	const authored = await screens({ kind: 'InspectScreen', screen: created.screen });
	assert.equal(
		authored.definition.target.display,
		display.id,
		'presentation preserves the authored identity'
	);
	const manager = page.getByRole('region', { name: 'Screens', exact: true });
	const tab = (type) => page.locator(`.gc-dv-tab[data-panel-type="${type}"]`);
	const title = async (type, expected) =>
		page.waitForFunction(
			({ type, expected }) =>
				document.querySelector(`.gc-dv-tab[data-panel-type="${type}"] .gc-dv-tab-label`)
					?.textContent === expected,
			{ type, expected }
		);
	await tab('screenManager').click();
	await manager.getByRole('button', { name: 'Identity screen', exact: true }).click();
	await manager
		.locator('[aria-label="Screen output details"]')
		.filter({ hasText: row.detail })
		.waitFor();
	await tab('screenWarp').click();
	await title('screenWarp', 'Screen Warping: Identity screen');
	await screens({ kind: 'RenameScreen', screen: created.screen, label: 'Renamed screen' });
	await title('screenWarp', 'Screen Warping: Renamed screen');
	const surface = (await screens({ kind: 'InspectSurface', reference: created.surface })).snapshot;
	await tab('screenManager').click();
	await manager.getByRole('button', { name: surface.label, exact: true }).click();
	await tab('surfaceSource').click();
	await title('surfaceSource', `Surface Source: ${surface.label}`);
	await screens({ kind: 'RenameSurface', reference: created.surface, label: 'Renamed surface' });
	await title('surfaceSource', 'Surface Source: Renamed surface');
	await screens({ kind: 'RemoveSurface', reference: created.surface });
	await title('surfaceSource', 'Surface Source');
	await tab('screenWarp').click();
	await title('screenWarp', 'Screen Warping: Renamed screen');
	await screens({ kind: 'RemoveScreen', screen: created.screen });
	await title('screenWarp', 'Screen Warping');
	const replacement = await screens({ kind: 'CreateScreen', label: 'Replacement screen' });
	await screens({
		kind: 'ConfigureScreen',
		screen: replacement.screen,
		definition: { enabled: false, target: { kind: 'Fullscreen', display: display.id } }
	});
	await tab('screenManager').click();
	await manager.getByRole('button', { name: 'Replacement screen', exact: true }).click();
	await tab('screenWarp').click();
	await title('screenWarp', 'Screen Warping: Replacement screen');
	await post('/api/ui/project-new', {});
	await title('screenWarp', 'Screen Warping');
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Icon target'
	});
	await edit({ kind: 'CreateReference', target: reference.media, label: 'Reference identity' });
	await tab('mediaManager').click();
	const referenceButton = page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: 'Reference identity', exact: true });
	const referenceIcon = referenceButton
		.locator('xpath=ancestor::*[contains(@class,"outliner-item-content")][1]')
		.locator('img.outliner-item-icon');
	assert(await referenceIcon.evaluate((image) => image.complete && image.naturalWidth > 0));
	for (const type of [
		'mediaManager',
		'mediaPreview',
		'screenManager',
		'screenWarp',
		'surfaceSource',
		'matrix',
		'composition'
	]) {
		assert(
			await tab(type)
				.locator('img.gc-dv-tab-icon')
				.evaluate((image) => image.complete && image.naturalWidth > 0),
			type
		);
	}
	await verifyRuntimeFooter({ base: options.base, post });
	await verifyRuntimeFooterRecovery({ page });
	assert.deepEqual(errors, []);
	console.log(
		JSON.stringify({
			screenIdentity:
				'native human-readable display row with exact authored ID; Screen/Surface selection, rename, deletion, replacement and New; loaded Reference and seven panel icons; headless unavailable footer',
			displayName: row.detail,
			pageErrors: errors
		})
	);
	if (process.env.MAPGYVER_IDENTITY_REVIEW === '1') {
		const reviewFile = path.join(identityArtifacts, `review-${options.nativePid}.done`);
		const review = {
			pid: options.nativePid,
			reviewFile,
			message: 'Owned identity check ready for taskbar review',
			expiresAt: new Date(Date.now() + 8 * 60000).toISOString()
		};
		await writeFile(path.join(identityArtifacts, 'manual-review.json'), JSON.stringify(review));
		console.log(JSON.stringify(review));
		const deadline = Date.now() + 8 * 60000;
		while (Date.now() < deadline) {
			try {
				await access(reviewFile);
				break;
			} catch (error) {
				if (error.code !== 'ENOENT') throw error;
			}
			await delay(500);
		}
	}
}
