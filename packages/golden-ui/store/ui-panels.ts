import type { PanelHandle, PanelQuery, PanelSpawnRequest } from '../dockview/panel-types';
import { appState } from './workbench.svelte';

export const accessPanel = (query: PanelQuery): PanelHandle | null => {
	return appState.panels?.accessPanel(query) ?? null;
};

export const getPanelById = (panelId: string): PanelHandle | null => {
	return appState.panels?.getPanelById(panelId) ?? null;
};

export const getPanelByType = (panelType: string): PanelHandle | null => {
	return appState.panels?.getPanelByType(panelType) ?? null;
};

export const showPanel = (request: PanelSpawnRequest): PanelHandle | null => {
	return appState.panels?.showPanel(request) ?? null;
};
