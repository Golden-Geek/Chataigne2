<script lang="ts">
	// The view's owner supplies one source key and the same interaction layer for each host.
	import type { ComponentProps } from 'svelte';
	import type { UiBrowserPreviewDependency, UiBrowserPreviewExtent } from 'golden_ui';
	import { hasNativePresentation } from 'golden_ui/host/desktop';
	import BrowserPreview from './BrowserPreview.svelte';
	import BrowserPreviewTiles from './BrowserPreviewTiles.svelte';
	import NativePreview from './NativePreview.svelte';
	let {
		browserFill,
		onextent,
		ondependencies,
		...view
	}: ComponentProps<typeof NativePreview> & {
		browserFill?: boolean;
		onextent?(extent: UiBrowserPreviewExtent): void;
		ondependencies?(dependencies: readonly UiBrowserPreviewDependency[]): void;
	} = $props();
</script>

{#if hasNativePresentation()}
	<NativePreview {...view} />
{:else if view.tileGroups}
	<BrowserPreviewTiles
		groups={view.tileGroups}
		label={view.label ?? 'Preview'}
		visible={view.visible}
		lifetime={view.lifetime}
	/>
{:else}
	<BrowserPreview
		source={view.source}
		label={view.label ?? 'Preview'}
		visible={view.visible}
		lifetime={view.lifetime}
		fill={browserFill}
		{onextent}
		{ondependencies}
		children={view.children}
	/>
{/if}
