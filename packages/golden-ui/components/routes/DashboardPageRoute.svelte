<script lang="ts">
	import type { NodeId } from '../../types';

	import DashboardViewer from '../panels/dashboard/DashboardViewer.svelte';

	let {
		pageSegment,
		hidePages = false,
		dashboardNodeId = undefined,
		routeBasePath = '/dashboard'
	} = $props<{
		pageSegment: string;
		hidePages?: boolean;
		dashboardNodeId?: NodeId;
		routeBasePath?: string;
	}>();
</script>

<DashboardViewer {pageSegment} {hidePages} {dashboardNodeId} {routeBasePath} />
