<script lang="ts">
	import { onMount } from 'svelte';
	import {
		MainWindow,
		useApplicationHost,
		registerNodeInspector,
		registerActionIcon,
		type PanelSpawnRequest,
		type UserPanelDefinitionMap
	} from 'golden_ui';
	import { providePreview } from '../lib/preview/context.svelte';
	import { provideApplicationRuntime } from '../lib/applications/runtime-context.svelte';
	import MediaManagerPanel from '../lib/media/MediaManagerPanel.svelte';
	import MediaPreviewPanel from '../lib/media/MediaPreviewPanel.svelte';
	import ScreenManagerPanel from '../lib/screens/ScreenManagerPanel.svelte';
	import ScreenWarpPanel from '../lib/screens/ScreenWarpPanel.svelte';
	import SurfaceSourcePanel from '../lib/screens/SurfaceSourcePanel.svelte';
	import CompositionPanel from '../lib/composition/CompositionPanel.svelte';
	import MatrixPanel from '../lib/matrix/MatrixPanel.svelte';
	import DomainInspector from '../lib/DomainInspector.svelte';
	import VideoRuntimeFooter from '../lib/runtime/VideoRuntimeFooter.svelte';
	import { provideVideoStatus } from '../lib/runtime/context.svelte';
	import { registerParameterVisibility } from '../lib/parameter-visibility';
	import { registerSenderAdornment } from '../lib/media/sender-adornment';
	import { ASSET_NODE_TYPE } from '../lib/media/generated/constants';
	import { registerMatrixContextMenu } from '../lib/matrix/context-menu';
	import { registerChannelContextMenu } from '../lib/media/context-menu';
	import removeIcon from 'golden_ui/style/icons/panels/close.svg';
	import '../lib/workbench.css';
	import { treeIcons, panelIcons } from '../lib/icons/tree';
	import { provideDocumentSelection } from '../lib/selection.svelte';
	const host = useApplicationHost(),
		preview = providePreview();
	provideDocumentSelection();
	provideVideoStatus();
	provideApplicationRuntime();
	registerParameterVisibility();
	registerSenderAdornment();
	registerMatrixContextMenu();
	registerChannelContextMenu();
	registerActionIcon('remove', removeIcon);
	onMount(() => preview.mount());
	for (const type of [
		'mapgyver.media.manager',
		'mapgyver.media.v8',
		'mapgyver.media.reference.v3',
		'mapgyver.media.channel',
		ASSET_NODE_TYPE,
		'mapgyver.media.folder',
		'mapgyver.screen',
		'mapgyver.surface',
		'mapgyver.app_control.v1'
	])
		registerNodeInspector(type, { component: DomainInspector });
	for (const type of ['mapgyver.matrix.layer.v3', 'mapgyver.matrix.group.v2'])
		registerNodeInspector(type, { childFilter: (child) => child.decl_id === 'parameters' });
	registerNodeInspector('mapgyver.matrix.column_group.v1', {
		childFilter: (child) => child.decl_id === 'app_control'
	});
	const userPanels: UserPanelDefinitionMap = {
		mediaManager: { title: 'Medias', icon: panelIcons.media, component: MediaManagerPanel },
		mediaPreview: { title: 'Media Preview', icon: panelIcons.media, component: MediaPreviewPanel },
		screenManager: { title: 'Screens', icon: panelIcons.screen, component: ScreenManagerPanel },
		screenWarp: { title: 'Screen Warping', icon: panelIcons.screen, component: ScreenWarpPanel },
		surfaceSource: {
			title: 'Surface Source',
			icon: panelIcons.surface,
			component: SurfaceSourcePanel
		},
		composition: {
			title: 'Composition',
			icon: panelIcons.composition,
			component: CompositionPanel
		},
		matrix: { title: 'Matrix', icon: panelIcons.matrix, component: MatrixPanel }
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
		},
		{
			panelId: 'media-preview',
			panelType: 'mediaPreview',
			position: { referencePanelId: 'screen-warp', direction: 'within' }
		}
	];
</script>

<div
	class="app"
	style:--gc-workbench-background={preview.state.transparent ? 'transparent' : undefined}>
	<MainWindow
		application={host.application}
		appIcon="/mapgyver-icon.png"
		{initialPanels}
		{userPanels}
		nodeIcons={treeIcons}
		httpBaseUrl={host.endpoints.httpBaseUrl}
		wsUrl={host.endpoints.wsUrl}>
		{#snippet footer()}<VideoRuntimeFooter />{/snippet}
	</MainWindow>
</div>

<style>
	.app {
		height: 100%;
		width: 100%;
	}
</style>
