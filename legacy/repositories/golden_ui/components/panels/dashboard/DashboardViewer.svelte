<script lang="ts">
	import { appState } from '../../../store/workbench.svelte';
	import type { NodeId, UiNodeDto } from '../../../types';

	import DashboardCanvas from './DashboardCanvas.svelte';
	import { getDirectItemChildren } from './dashboard-model';
	import { findDashboardPageByRouteSegment, getDashboardPageRouteHref } from './dashboard-route';

	let {
		pageSegment = null,
		hidePages = false,
		dashboardNodeId = undefined,
		routeBasePath = '/dashboard'
	} = $props<{
		pageSegment?: string | null;
		hidePages?: boolean;
		dashboardNodeId?: NodeId;
		routeBasePath?: string;
	}>();

	let session = $derived(appState.session);
	let graph = $derived(session?.graph.state ?? null);
	let isLoading = $derived.by(() => {
		if (!session) {
			return true;
		}
		return session.status !== 'connected' && graph?.rootId === null;
	});

	let dashboards = $derived.by((): UiNodeDto[] => {
		if (!graph) {
			return [];
		}
		return [...graph.nodesById.values()]
			.filter((candidate) => candidate.node_type === 'dashboard')
			.sort((left, right) => left.meta.label.localeCompare(right.meta.label));
	});

	let selectedDashboard = $derived.by((): UiNodeDto | null => {
		if (dashboards.length === 0) {
			return null;
		}
		if (dashboardNodeId === undefined) {
			return dashboards[0];
		}
		return dashboards.find((candidate) => candidate.node_id === dashboardNodeId) ?? dashboards[0];
	});

	let pages = $derived.by(() => getDirectItemChildren(graph, selectedDashboard, 'dashboard_page'));
	let selectedPage = $derived.by((): UiNodeDto | null => {
		if (pages.length === 0) {
			return null;
		}
		if (!pageSegment) {
			return pages[0];
		}
		return findDashboardPageByRouteSegment(pages, pageSegment);
	});
	let showPageList = $derived(pages.length > 0 && !hidePages);
	let dashboardHomeHref = $derived(hidePages ? `${routeBasePath}?hidePages` : routeBasePath);

	const hrefForPage = (page: UiNodeDto): string =>
		getDashboardPageRouteHref(page, pages, {
			routeBasePath,
			hidePages
		});
</script>

<section class="dashboard-viewer">
	{#if isLoading}
		<div class="dashboard-viewer-empty">
			<h2>Loading dashboard</h2>
			<p>The viewer is waiting for the workspace snapshot.</p>
		</div>
	{:else if dashboards.length === 0}
		<div class="dashboard-viewer-empty">
			<h2>No dashboard nodes are available</h2>
			<p>Create a <code>dashboard</code> node to expose dashboard pages in the viewer route.</p>
		</div>
	{:else if pages.length === 0}
		<div class="dashboard-viewer-empty">
			<h2>No dashboard pages are available</h2>
			<p>The selected dashboard does not contain any pages yet.</p>
		</div>
	{:else}
		<div class="dashboard-viewer-shell">
			{#if showPageList}
				<nav class="dashboard-viewer-pages" aria-label="Dashboard pages">
					<div class="dashboard-viewer-pages-list">
						{#each pages as page}
							<a
								href={hrefForPage(page)}
								class:selected={selectedPage?.node_id === page.node_id}
								aria-current={selectedPage?.node_id === page.node_id ? 'page' : undefined}
								>{page.meta.label}</a>
						{/each}
					</div>
				</nav>
			{/if}

			<div class="dashboard-viewer-canvas">
				{#if selectedPage}
					<div class="dashboard-viewer-canvas-frame">
						{#key selectedPage.node_id}
							<DashboardCanvas node={selectedPage} editMode={false} pageNavigationEnabled={false} />
						{/key}
					</div>
				{:else}
					<div class="dashboard-viewer-empty inline dashboard-viewer-canvas-frame">
						<h2>Dashboard page not found</h2>
						<p>
							The route segment <code>{pageSegment}</code> does not match any page in this dashboard.
						</p>
						<a class="dashboard-viewer-backlink" href={dashboardHomeHref}>Back to dashboard</a>
					</div>
				{/if}
			</div>
		</div>
	{/if}
</section>

<style>
	.dashboard-viewer {
		inline-size: 100%;
		block-size: 100%;
		min-block-size: 100vh;
		min-inline-size: 0;
		box-sizing: border-box;
		background: var(--gc-color-background);
		color: var(--gc-color-text);
	}

	.dashboard-viewer-shell {
		display: flex;
		flex-direction: column;
		block-size: 100%;
		min-block-size: calc(100vh - 2.5rem);
		margin-inline: auto;
		min-inline-size: 0;
		min-block-size: 0;
	}

	.dashboard-viewer-pages {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 1rem;
		min-inline-size: 0;
		padding: 0.1rem 1rem;
	}

	.dashboard-viewer-pages-list {
		display: flex;
		flex: 1 1 24rem;
		gap: 0.15rem;
		min-inline-size: 0;
		overflow-x: auto;
		overflow-y: hidden;
		padding-block: 0.1rem;
		scrollbar-width: thin;
	}

	.dashboard-viewer-pages-list a,
	.dashboard-viewer-backlink {
		display: flex;
		align-items: center;
		flex: 0 0 auto;
		min-inline-size: 0;
		border-radius: 0.9rem;
		color: inherit;
		padding: 0.35rem 0.9rem;
		text-decoration: none;
		white-space: nowrap;
		font-size: 0.8rem;
		transition:
			background 200ms ease,
			border-color 120ms ease,
			transform 120ms ease;
	}

	.dashboard-viewer-pages-list a:hover,
	.dashboard-viewer-backlink:hover {
		background: rgb(from var(--gc-color-selection) r g b / 0.5);
		transform: translateY(-0.04rem);
	}

	.dashboard-viewer-pages-list a.selected {
		background: rgb(from var(--gc-color-selection) r g b / 0.3);
		color: white;
		box-shadow: 0 0.4rem 1rem rgb(from var(--gc-color-selection) r g b / 0.12);
	}

	.dashboard-viewer-canvas {
		display: flex;
		flex: 1 1 auto;
		min-inline-size: 0;
		min-block-size: 0;
	}

	.dashboard-viewer-canvas-frame {
		display: flex;
		flex: 1 1 auto;
		min-inline-size: 0;
		min-block-size: 0;
		overflow: hidden;
	}

	.dashboard-viewer-empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		inline-size: 100%;
		block-size: 100%;
		gap: 0.55rem;
		padding: 2rem;
		box-sizing: border-box;
		text-align: center;
	}

	.dashboard-viewer-empty.inline {
		min-block-size: 100%;
	}

	.dashboard-viewer-empty p {
		margin: 0;
		opacity: 0.78;
	}

	.dashboard-viewer-empty p {
		font-size: 0.82rem;
		line-height: 1.5;
		max-inline-size: 34rem;
	}

	.dashboard-viewer-empty :global(code) {
		padding: 0.12rem 0.35rem;
		background: rgb(from var(--gc-color-background) r g b / 0.55);
	}

	.dashboard-viewer-backlink {
		margin-top: 0.35rem;
	}
</style>
