<script lang="ts">
	import { onMount } from 'svelte';
	import {
		MainWindow,
		useApplicationHost,
		registerNodeInspector,
		type PanelSpawnRequest,
		type UserPanelDefinitionMap
	} from 'golden_ui';
	import { providePreview } from '../lib/preview/context.svelte';
	import { provideScreenAuthoring } from '../lib/screens/context.svelte';
	import { provideCompositionAuthoring } from '../lib/composition/context.svelte';
	import { provideMatrixAuthoring } from '../lib/matrix/context.svelte';
	import MediaManagerPanel from '../lib/media/MediaManagerPanel.svelte';
	import ScreenManagerPanel from '../lib/screens/ScreenManagerPanel.svelte';
	import ScreenWarpPanel from '../lib/screens/ScreenWarpPanel.svelte';
	import SurfaceSourcePanel from '../lib/screens/SurfaceSourcePanel.svelte';
	import CompositionPanel from '../lib/composition/CompositionPanel.svelte';
	import MatrixPanel from '../lib/matrix/MatrixPanel.svelte';
	import ApplicationPanel from '../lib/applications/ApplicationPanel.svelte';
	import DomainInspector from '../lib/DomainInspector.svelte';
	import { registerParameterVisibility } from '../lib/parameter-visibility';
	import { registerMatrixContextMenu } from '../lib/matrix/context-menu';
	import '../lib/workbench.css';
	import { treeIcons } from '../lib/icons/tree';
	import { provideDocumentSelection } from '../lib/selection.svelte';
	const host = useApplicationHost(),
		preview = providePreview();
	provideDocumentSelection();
	provideScreenAuthoring();
	provideCompositionAuthoring();
	provideMatrixAuthoring();
	registerParameterVisibility();
	registerMatrixContextMenu();
	onMount(() => preview.mount());
	for (const type of [
		'mapgyver.media.manager',
		'mapgyver.media',
		'mapgyver.media.channel',
		'mapgyver.media.asset',
		'mapgyver.media.folder',
		'mapgyver.screen',
		'mapgyver.surface',
		'mapgyver.matrix.layer',
		'mapgyver.matrix.group',
		'mapgyver.matrix.column',
		'mapgyver.application'
	])
		registerNodeInspector(type, { component: DomainInspector });
	const userPanels: UserPanelDefinitionMap = {
		mediaManager: { title: 'Media Manager', component: MediaManagerPanel },
		screenManager: { title: 'Screen Manager', component: ScreenManagerPanel },
		screenWarp: { title: 'Screen Warping', component: ScreenWarpPanel },
		surfaceSource: { title: 'Surface Source', component: SurfaceSourcePanel },
		composition: { title: 'Composition', component: CompositionPanel },
		matrix: { title: 'Matrix', component: MatrixPanel },
		applications: { title: 'Applications', component: ApplicationPanel }
	};
	const initialPanels: PanelSpawnRequest[] = [
		{
			panelId: 'media-manager',
			panelType: 'mediaManager',
			required: true,
			initialWidth: typeof window === 'undefined' ? undefined : window.innerWidth * 0.2
		},
		{
			panelId: 'screen-manager',
			panelType: 'screenManager',
			required: true,
			position: { referencePanelId: 'media-manager', direction: 'within' }
		},
		{
			panelId: 'applications',
			panelType: 'applications',
			required: true,
			position: { referencePanelId: 'media-manager', direction: 'within' }
		},
		{
			panelId: 'screen-warp',
			panelType: 'screenWarp',
			required: true,
			initialWidth: typeof window === 'undefined' ? undefined : window.innerWidth * 0.8,
			position: { referencePanelId: 'media-manager', direction: 'right' }
		},
		{
			panelId: 'surface-source',
			panelType: 'surfaceSource',
			required: true,
			position: { referencePanelId: 'screen-warp', direction: 'within' }
		},
		{
			panelId: 'composition',
			panelType: 'composition',
			required: true,
			position: { referencePanelId: 'screen-warp', direction: 'within' }
		},
		{
			panelId: 'matrix',
			panelType: 'matrix',
			required: true,
			position: { referencePanelId: 'screen-warp', direction: 'within' }
		},
		{
			panelId: 'inspector',
			panelType: 'inspector',
			initialWidth: typeof window === 'undefined' ? undefined : window.innerWidth * 0.2,
			position: { referencePanelId: 'screen-warp', direction: 'right' }
		}
	];
</script>

<div
	class="app"
	style:--gc-workbench-background={preview.state.transparent ? 'transparent' : undefined}>
	<MainWindow
		application={host.application}
		{initialPanels}
		{userPanels}
		nodeIcons={treeIcons}
		httpBaseUrl={host.endpoints.httpBaseUrl}
		wsUrl={host.endpoints.wsUrl} />
</div>

<style>
	.app {
		height: 100%;
		width: 100%;
	}
</style>
