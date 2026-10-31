<script lang="ts">
	import { onMount } from 'svelte';
	import { providePreview } from '../lib/preview/context.svelte';
	import {
		MainWindow,
		useApplicationHost,
		type PanelSpawnRequest,
		type UserPanelDefinitionMap
	} from 'golden_ui';
	import MediaManagerPanel from '../lib/media/MediaManagerPanel.svelte';
	import ScreenManagerPanel from '../lib/screens/ScreenManagerPanel.svelte';
	import SurfaceSourcePanel from '../lib/screens/SurfaceSourcePanel.svelte';
	import CompositionPanel from '../lib/composition/CompositionPanel.svelte';
	import MatrixPanel from '../lib/matrix/MatrixPanel.svelte';
	import { provideScreenAuthoring } from '../lib/screens/context.svelte';

	const host = useApplicationHost();
	const preview = providePreview();
	provideScreenAuthoring();
	onMount(() => preview.mount());
	const userPanels: UserPanelDefinitionMap = {
		mediaManager: { title: 'Media Manager', component: MediaManagerPanel },
		screenManager: { title: 'Screen Manager', component: ScreenManagerPanel },
		surfaceSource: { title: 'Surface Source', component: SurfaceSourcePanel },
		composition: { title: 'Composition', component: CompositionPanel },
		matrix: { title: 'Matrix', component: MatrixPanel }
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
			panelId: 'surface-source',
			panelType: 'surfaceSource',
			required: true,
			position: { referencePanelId: 'screen-manager', direction: 'within' }
		},
		{
			panelId: 'composition',
			panelType: 'composition',
			required: true,
			position: { referencePanelId: 'media-manager', direction: 'within' }
		},
		{
			panelId: 'logger',
			panelType: 'logger',
			position: { referencePanelId: 'media-manager', direction: 'below' }
		},
		{
			panelId: 'matrix',
			panelType: 'matrix',
			required: true,
			position: { referencePanelId: 'media-manager', direction: 'within' }
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
