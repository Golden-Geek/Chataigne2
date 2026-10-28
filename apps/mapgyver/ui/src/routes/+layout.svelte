<script lang="ts">
	import { dev } from '$app/env';
	import { ApplicationRoot } from 'golden_ui';

	let { children } = $props();
	const developmentBackendPort = dev
		? Number(import.meta.env.VITE_GC_UI_BACKEND_PORT ?? 7011)
		: undefined;
</script>

<svelte:head>
	<link rel="icon" href="/favicon.svg" />
</svelte:head>

<ApplicationRoot expectedApplicationId="mapgyver" development={dev} {developmentBackendPort}>
	{@render children()}
</ApplicationRoot>
