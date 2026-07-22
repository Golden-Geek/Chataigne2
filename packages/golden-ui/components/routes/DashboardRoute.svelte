<script lang="ts">
	import type { NodeId } from '../../types';

	import DashboardViewer from '../panels/dashboard/DashboardViewer.svelte';

	let { dashboardNodeId = undefined, routeBasePath = '/dashboard' } = $props<{
		dashboardNodeId?: NodeId;
		routeBasePath?: string;
	}>();
</script>

<DashboardViewer {dashboardNodeId} {routeBasePath} />
