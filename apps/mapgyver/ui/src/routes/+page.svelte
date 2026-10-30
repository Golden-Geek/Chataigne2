<script lang="ts">
	import {
		MainWindow,
		useApplicationHost,
		type PanelSpawnRequest,
		type UserPanelDefinitionMap
	} from 'golden_ui';
	import MediaManagerPanel from '../lib/media/MediaManagerPanel.svelte';

	const host = useApplicationHost();
	const userPanels: UserPanelDefinitionMap = {
		mediaManager: { title: 'Media Manager', component: MediaManagerPanel }
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

<MainWindow
	application={host.application}
	{initialPanels}
	{userPanels}
	httpBaseUrl={host.endpoints.httpBaseUrl}
	wsUrl={host.endpoints.wsUrl} />
