<script lang="ts">
	import favicon from '$lib/assets/favicon.svg';
	import { browser, dev } from '$app/environment';
	import RuntimeProbeOverlay from '$lib/golden_ui/components/debug/RuntimeProbeOverlay.svelte';
	import { ensureRuntimeProbeInstalled } from '$lib/golden_ui/debug/runtime-probe.svelte';

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

{@render children()}

{#if dev}
	<RuntimeProbeOverlay />
{/if}
