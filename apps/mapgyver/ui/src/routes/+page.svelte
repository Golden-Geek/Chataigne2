<script lang="ts">
	import { onMount } from 'svelte';
	import { provideMediaPreview } from '../lib/media/preview-context.svelte';
	import {
		MainWindow,
		useApplicationHost,
		type PanelSpawnRequest,
		type UserPanelDefinitionMap
	} from 'golden_ui';
	import MediaManagerPanel from '../lib/media/MediaManagerPanel.svelte';
	import ScreenManagerPanel from '../lib/screens/ScreenManagerPanel.svelte';

	const host = useApplicationHost();
	const preview = provideMediaPreview();
	onMount(() => preview.mount());
	const userPanels: UserPanelDefinitionMap = {
		mediaManager: { title: 'Media Manager', component: MediaManagerPanel },
		screenManager: { title: 'Screen Manager', component: ScreenManagerPanel }
	};

	const initialPanels: PanelSpawnRequest[] = [
		{ panelId: 'outliner', panelType: 'outliner' },
		{
			panelId: 'media-manager',
			panelType: 'mediaManager',
			required: true,
			position: { referencePanelId: 'outliner', direction: 'right' }
		},
		{
			panelId: 'screen-manager',
			panelType: 'screenManager',
			required: true,
			position: { referencePanelId: 'media-manager', direction: 'within' }
		},
		{
			panelId: 'logger',
			panelType: 'logger',
			position: { referencePanelId: 'media-manager', direction: 'below' }
		},
		{
			panelId: 'inspector',
			panelType: 'inspector',
			position: { referencePanelId: 'media-manager', direction: 'right' }
		}
	];
</script>

<div
	class="app"
	style:--gc-workbench-background={preview.state.transparent ? 'transparent' : undefined}
	style:--gc-dock-panel-background={preview.state.transparent ? 'transparent' : undefined}
	style:--mapgyver-panel-background={preview.state.transparent ? 'transparent' : undefined}>
	<MainWindow
		application={host.application}
		{initialPanels}
		{userPanels}
		httpBaseUrl={host.endpoints.httpBaseUrl}
		wsUrl={host.endpoints.wsUrl} />
</div>

<style>
	.app {
		height: 100%;
		width: 100%;
	}
</style>
