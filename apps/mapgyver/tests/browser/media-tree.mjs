import assert from 'node:assert/strict';

/** Inspect a real channel through its Channels row and shared selection action. */
export async function selectMediaChannel(page, catalog, reference) {
	const media = catalog.media.find((item) => item.id === reference.media);
	assert(media);
	const channelIndex = media.channels.findIndex(
		(item) => item.reference.channel === reference.channel
	);
	assert(
		channelIndex >= 0 && channelIndex < 32,
		'fixture channel must be on its first bounded page'
	);
	await page.getByRole('tab').first().click();
	const manager = page.locator('section[data-panel-id="media-manager"]');
	const parent = manager.getByRole('treeitem', { name: media.label, exact: true });
	await parent.getByRole('button', { name: media.label, exact: true }).click();
	const channels = page.getByRole('region', { name: 'Channels', exact: true });
	const rows = channels.locator(
		':scope > .node-inspector-children > .node-inspector-children-wrapper > .node-inspector'
	);
	await rows
		.nth(channelIndex)
		.locator(':scope > .node-header > .node-title')
		.click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Watch in Inspector', exact: true }).click();
}
