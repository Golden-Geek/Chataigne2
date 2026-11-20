import { appState, type UiBrowserPreviewExtent } from 'golden_ui';
import { usePreview } from './context.svelte';
import { previewKey } from './identity';
import type { PreviewSource } from './generated/PreviewSource';

export function useViewSource(readSource: () => PreviewSource | null) {
	const owner = usePreview();
	const source = $derived(readSource());
	const key = $derived(source ? previewKey(source) : null);
	const wire = $derived(JSON.stringify(source));
	const project = $derived(appState.session?.projectGeneration ?? 0);
	const status = $derived(key ? owner.statuses.get(key) : undefined);
	let browser = $state<{ key: string; project: number; extent: UiBrowserPreviewExtent } | null>(
		null
	);
	return {
		get wire() {
			return wire;
		},
		get status() {
			return status;
		},
		get extent() {
			return (
				status?.extent ??
				(browser?.key === key && browser.project === project ? browser.extent : undefined)
			);
		},
		onextent(extent: UiBrowserPreviewExtent) {
			if (key) browser = { key, project, extent };
		}
	};
}
