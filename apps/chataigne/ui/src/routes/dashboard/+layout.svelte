<script lang="ts">
	import { page } from '$app/state';
	import { DashboardRouteLayout, DashboardViewer } from 'golden_ui';
	import type { Snippet } from 'svelte';

	let { children } = $props<{ children: Snippet }>();

	const pageSegment = $derived(page.params.page ?? null);
	const hidePages = $derived(page.url.searchParams.has('hidePages'));
</script>

<DashboardRouteLayout>
	<DashboardViewer {pageSegment} {hidePages} />
	{@render children()}
</DashboardRouteLayout>
