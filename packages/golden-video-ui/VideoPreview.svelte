<script lang="ts">
	// The view's owner supplies one source key and the same interaction layer for each host.
	import type { ComponentProps } from 'svelte';
	import type { UiBrowserPreviewExtent } from 'golden_ui';
	import { hasDesktopHost } from 'golden_ui/host/desktop';
	import BrowserPreview from './BrowserPreview.svelte';
	import NativePreview from './NativePreview.svelte';
	let {
		browserFill,
		onextent,
		...view
	}: ComponentProps<typeof NativePreview> & {
		browserFill?: boolean;
		onextent?(extent: UiBrowserPreviewExtent): void;
	} = $props();
</script>

{#if hasDesktopHost()}
	<NativePreview {...view} />
{:else}
	<BrowserPreview
		source={view.source}
		label={view.label ?? 'Preview'}
		visible={view.visible}
		lifetime={view.lifetime}
		fill={browserFill}
		{onextent}
		children={view.children}
	/>
{/if}
