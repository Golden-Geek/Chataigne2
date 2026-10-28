<script lang="ts">
	import favicon from '#lib/assets/favicon.svg';
	import { browser, dev } from '$app/env';
	import { ApplicationRoot } from 'golden_ui';
	import RuntimeProbeOverlay from 'golden_ui/components/debug/RuntimeProbeOverlay.svelte';
	import { ensureRuntimeProbeInstalled } from 'golden_ui/debug/runtime-probe.svelte';

	let { children } = $props();

	$effect(() => {
		if (!browser || !dev) {
			return;
		}
		ensureRuntimeProbeInstalled();
	});
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<ApplicationRoot
	expectedApplicationId="chataigne"
	development={dev}
	developmentBackendPort={Number(import.meta.env.VITE_GC_UI_BACKEND_PORT ?? 7010)}>
	{@render children()}
</ApplicationRoot>

{#if dev}
	<RuntimeProbeOverlay />
{/if}
