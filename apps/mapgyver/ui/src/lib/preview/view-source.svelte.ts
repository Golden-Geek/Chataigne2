import { appState, type UiBrowserPreviewDependency, type UiBrowserPreviewExtent } from 'golden_ui';
import { usePreview } from './context.svelte';
import { previewKey } from './identity';
import type { PreviewSource } from './generated/PreviewSource';
import type { ChannelReference } from '../media/generated/ChannelReference';

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

/** Intrinsic geometry from the installed plan for this editor and project. */
export function useDependentExtents(ownerId: () => string) {
	const owner = usePreview();
	const scope = $derived(`${appState.session?.projectGeneration ?? 0}:${ownerId()}`);
	let browser = $state.raw<{ scope: string; values: Map<string, UiBrowserPreviewExtent> } | null>(
		null
	);
	return {
		extent(reference: ChannelReference | null) {
			if (!reference) return null;
			const key = previewKey({ kind: 'Channel', reference });
			return (
				owner.statuses.get(key)?.extent ??
				(browser?.scope === scope ? browser.values.get(key) : null) ??
				null
			);
		},
		update(dependencies: readonly UiBrowserPreviewDependency[]) {
			const values = new Map<string, UiBrowserPreviewExtent>();
			for (const dependency of dependencies) {
				const source = JSON.parse(dependency.source) as PreviewSource;
				if (source.kind === 'Channel') values.set(previewKey(source), dependency.extent);
			}
			browser = { scope, values };
		}
	};
}
