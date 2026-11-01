import assert from 'node:assert/strict';

/** Activate a real channel row using its observed parent and sibling order, even with duplicate labels. */
export async function selectMediaChannel(page, catalog, reference) {
	const media = catalog.media.find((item) => item.id === reference.media);
	assert(media);
	const channelIndex = media.channels.findIndex(
		(item) => item.reference.channel === reference.channel
	);
	assert(channelIndex >= 0);
	await page
		.getByRole('tab')
		.filter({ hasText: /^Media Manager$/ })
		.click();
	const manager = page.locator('section[aria-label="Media Manager"]');
	const parent = manager.getByRole('treeitem', { name: media.label, exact: true });
	await parent.getByRole('button', { name: media.label, exact: true }).click();
	if ((await parent.getAttribute('aria-expanded')) === 'false') await parent.press('ArrowRight');
	const rows = manager.getByRole('treeitem');
	const labels = await rows.evaluateAll((elements) =>
		elements.map((element) => element.getAttribute('aria-label'))
	);
	const index = labels.indexOf(media.label);
	assert(index >= 0);
	const child = rows.nth(index + channelIndex + 1);
	assert.equal(
		Number(await child.getAttribute('aria-level')),
		Number(await parent.getAttribute('aria-level')) + 1
	);
	await child
		.getByRole('button', { name: media.channels[channelIndex].label, exact: true })
		.click();
}
