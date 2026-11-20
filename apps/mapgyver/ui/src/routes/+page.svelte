<script lang="ts">
	import { onMount } from 'svelte';
	import {
		MainWindow,
		FileExplorerPanel,
		useApplicationHost,
		registerNodeInspector,
		registerActionIcon,
		type PanelSpawnRequest,
		type UserPanelDefinitionMap
	} from 'golden_ui';
	import { hasDesktopHost } from 'golden_ui/host/desktop';
	import { BrowserPreviewQuality, provideBrowserPreviews } from 'golden_video_ui';
	import explorerIcon from 'golden_ui/style/icons/node/folder.svg';
	import { providePreview } from '../lib/preview/context.svelte';
	import { provideApplicationRuntime } from '../lib/applications/runtime-context.svelte';
	import MediaManagerPanel from '../lib/media/MediaManagerPanel.svelte';
	import MediaPreviewPanel from '../lib/media/MediaPreviewPanel.svelte';
	import ShaderEditorPanel from '../lib/shader/ShaderEditorPanel.svelte';
	import ShaderLibraryPanel from '../lib/shader/ShaderLibraryPanel.svelte';
	import shaderIcon from 'golden_ui/style/icons/node/script.svg';
	import ScreenManagerPanel from '../lib/screens/ScreenManagerPanel.svelte';
	import ScreenWarpPanel from '../lib/screens/ScreenWarpPanel.svelte';
	import SurfaceSourcePanel from '../lib/screens/SurfaceSourcePanel.svelte';
	import CompositionPanel from '../lib/spatial/SpatialPanel.svelte';
	import MatrixPanel from '../lib/matrix/MatrixPanel.svelte';
	import DomainInspector from '../lib/DomainInspector.svelte';
	import VideoRuntimeFooter from '../lib/runtime/VideoRuntimeFooter.svelte';
	import { provideVideoStatus } from '../lib/runtime/context.svelte';
	import { registerParameterVisibility } from '../lib/parameter-visibility';
	import { registerSenderAdornment } from '../lib/media/sender-adornment';
	import { registerMediaTargetPicker } from '../lib/media/target-picker';
	import { ASSET_NODE_TYPE } from '../lib/media/generated/constants';
	import { registerMatrixContextMenu } from '../lib/matrix/context-menu';
	import { registerMediaContextMenus } from '../lib/media/context-menu';
	import { registerCompositionContextMenu } from '../lib/composition/context-menu';
	import removeIcon from 'golden_ui/style/icons/panels/close.svg';
	import '../lib/workbench.css';
	import { treeIcons, panelIcons } from '../lib/icons/tree';
	import { provideDocumentSelection } from '../lib/selection.svelte';
	const host = useApplicationHost(),
		preview = providePreview();
	provideBrowserPreviews();
	provideDocumentSelection();
	provideVideoStatus();
	provideApplicationRuntime();
	registerParameterVisibility();
	registerSenderAdornment();
	registerMediaTargetPicker();
	registerMatrixContextMenu();
	registerMediaContextMenus();
	registerCompositionContextMenu();
	registerActionIcon('remove', removeIcon);
	onMount(() => preview.mount());
	for (const type of [
		'mapgyver.media.manager',
		'mapgyver.media.v11',
		'mapgyver.media.reference.v4',
		'mapgyver.media.channel.v2',
		ASSET_NODE_TYPE,
		'mapgyver.media.folder',
		'mapgyver.screen',
		'mapgyver.surface',
		'mapgyver.app_control.v1'
	])
		registerNodeInspector(type, { component: DomainInspector });
	for (const type of ['mapgyver.matrix.layer.v4', 'mapgyver.matrix.group.v3'])
		registerNodeInspector(type, { childFilter: (child) => child.decl_id === 'parameters' });
	registerNodeInspector('mapgyver.matrix.column_group.v1', {
		childFilter: (child) => child.decl_id === 'parameters' || child.decl_id === 'app_control'
	});
	const userPanels: UserPanelDefinitionMap = {
		mediaManager: { title: 'Medias', icon: panelIcons.media, component: MediaManagerPanel },
		mediaPreview: { title: 'Media Preview', icon: panelIcons.media, component: MediaPreviewPanel },
		shaderEditor: { title: 'Shader Editor', icon: shaderIcon, component: ShaderEditorPanel },
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
		matrix: { title: 'Matrix', icon: panelIcons.matrix, component: MatrixPanel },
		...(hasDesktopHost()
			? {
					fileExplorer: {
						title: 'File Explorer',
						icon: explorerIcon,
						component: FileExplorerPanel
					},
					shaderLibrary: {
						title: 'Shader Library',
						icon: shaderIcon,
						component: ShaderLibraryPanel
					}
				}
			: {})
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
		},
		{
			panelId: 'shader-editor',
			panelType: 'shaderEditor',
			inactive: true,
			position: { referencePanelId: 'screen-warp', direction: 'within' }
		},
		...(hasDesktopHost()
			? [
					{
						panelId: 'file-explorer',
						panelType: 'fileExplorer',
						position: { referencePanelId: 'media-manager', direction: 'below' as const }
					},
					{
						panelId: 'shader-library',
						panelType: 'shaderLibrary',
						inactive: true,
						position: { referencePanelId: 'file-explorer', direction: 'within' as const }
					}
				]
			: [])
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
		{#snippet header()}{#if !hasDesktopHost()}<BrowserPreviewQuality />{/if}{/snippet}
		{#snippet footer()}<VideoRuntimeFooter />{/snippet}
	</MainWindow>
</div>

<style>
	.app {
		height: 100%;
		width: 100%;
	}
</style>
