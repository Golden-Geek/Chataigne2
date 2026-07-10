import type { PanelController } from '../dockview/panel-types';
import type { WorkbenchSession } from './workbench.svelte';

export const appState = $state({
	session: null as WorkbenchSession | null,
	panels: null as PanelController | null
});
