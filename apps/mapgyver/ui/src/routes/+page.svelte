<script lang="ts">
	import { MainWindow, useApplicationHost, type PanelSpawnRequest } from 'golden_ui';

	const host = useApplicationHost();

	const initialPanels: PanelSpawnRequest[] = [
		{ panelId: 'outliner', panelType: 'outliner' },
		{
			panelId: 'dashboard',
			panelType: 'dashboard',
			position: { referencePanelId: 'outliner', direction: 'right' }
		},
		{
			panelId: 'logger',
			panelType: 'logger',
			position: { referencePanelId: 'dashboard', direction: 'below' }
		},
		{
			panelId: 'inspector',
			panelType: 'inspector',
			position: { referencePanelId: 'dashboard', direction: 'right' }
		}
	];
</script>

<MainWindow
	application={host.application}
	{initialPanels}
	httpBaseUrl={host.endpoints.httpBaseUrl}
	wsUrl={host.endpoints.wsUrl} />
