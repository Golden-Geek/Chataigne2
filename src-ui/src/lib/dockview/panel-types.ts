import type { DockviewPanelApi } from "dockview-core";

export type DockPanelParams = Record<string, unknown>;

export interface DockPanelState {
	panelId: string;
	title: string;
	params: DockPanelParams;
}

export interface DockPanelProps extends DockPanelState {
	api: DockviewPanelApi;
}

export interface DockPanelExports {
	setDockPanelState: (next: DockPanelState) => void;
}
