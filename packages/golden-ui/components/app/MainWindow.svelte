<script lang="ts">
	import MainComponent from './MainComponent.svelte';
	import AppHeader from './AppHeader.svelte';
	import { appState } from '../../store/workbench.svelte';
	import AppFooter from './AppFooter.svelte';
	import type { PanelSpawnRequest, UserPanelDefinitionMap } from '../../dockview/panel-types';
	import type { NodeIconSet } from '../../store/node-types';
	import { platform } from '../../store/platform.svelte';
	import WorkbenchSessionRoot from './WorkbenchSessionRoot.svelte';
	import WorkbenchOverlayHost from './WorkbenchOverlayHost.svelte';
	import type { UiApplicationInfoDto } from '../../generated/rust_protocol/UiApplicationInfoDto';

	const props = $props<{
		application: UiApplicationInfoDto;
		wsUrl?: string;
		httpBaseUrl?: string;
		bootstrapRetryMs?: number;
		userPanels?: UserPanelDefinitionMap;
		initialPanels?: PanelSpawnRequest[];
		nodeIcons?: NodeIconSet;
	}>();
	let session = $derived(appState.session);
</script>

<WorkbenchSessionRoot
	application={props.application}
	wsUrl={props.wsUrl}
	httpBaseUrl={props.httpBaseUrl}
	bootstrapRetryMs={props.bootstrapRetryMs}>
	{#if session}
		<div
			class="gc-main os-{platform.name}"
			data-graph-node-count={session.graph.state.nodesById.size}
			data-graph-event-tick={session.graph.state.lastEventTime?.tick ?? 0}>
			<AppHeader {session} application={props.application} />
			<MainComponent
				userPanels={props.userPanels}
				initialPanels={props.initialPanels}
				nodeIcons={props.nodeIcons} />
			<WorkbenchOverlayHost />
			<AppFooter {session} />
		</div>
	{/if}
</WorkbenchSessionRoot>
