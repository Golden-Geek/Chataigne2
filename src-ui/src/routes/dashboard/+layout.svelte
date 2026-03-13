<script lang="ts">
	import { page } from '$app/state';
	import { DashboardRouteLayout, DashboardViewer } from 'golden_ui';

	const pageSegment = $derived(page.params.page ?? null);
	const hidePages = $derived(page.url.searchParams.has('hidePages'));
</script>

<DashboardRouteLayout>
	<DashboardViewer {pageSegment} {hidePages} />
</DashboardRouteLayout>
