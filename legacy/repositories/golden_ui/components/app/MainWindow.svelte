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

	const props = $props<{
		wsUrl?: string;
		httpBaseUrl?: string;
		pollIntervalMs?: number;
		bootstrapRetryMs?: number;
		userPanels?: UserPanelDefinitionMap;
		initialPanels?: PanelSpawnRequest[];
		nodeIcons?: NodeIconSet;
	}>();
	let session = $derived(appState.session);
</script>

<WorkbenchSessionRoot
	wsUrl={props.wsUrl}
	httpBaseUrl={props.httpBaseUrl}
	pollIntervalMs={props.pollIntervalMs}
	bootstrapRetryMs={props.bootstrapRetryMs}>
	{#if session}
		<div class="gc-main os-{platform.name}">
			<AppHeader {session} />
			<MainComponent
				userPanels={props.userPanels}
				initialPanels={props.initialPanels}
				nodeIcons={props.nodeIcons} />
			<WorkbenchOverlayHost />
			<AppFooter {session} />
		</div>
	{/if}
</WorkbenchSessionRoot>
