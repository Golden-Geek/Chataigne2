import type { Component } from 'svelte';
import type { DockviewPanelRenderer } from 'dockview-core';

export type PanelParams = Record<string, unknown>;
export type PanelDockDirection = 'within' | 'left' | 'right' | 'above' | 'below';
export type PanelRenderPolicy = DockviewPanelRenderer;

export interface PanelState {
	panelId: string;
	panelType: string;
	title: string;
	params: PanelParams;
}

export interface PanelApi {
	setTitle: (title: string) => void;
	close: () => void;
	updateParams: (params: PanelParams) => void;
	getParams: <T extends PanelParams = PanelParams>() => T;
}

export interface PanelProps extends PanelState {
	panelApi: PanelApi;
}

export interface PanelExports {
	setPanelState: (next: PanelState) => void;
}

export type PanelComponent = Component<PanelProps, PanelExports>;

export interface PanelDefinition {
	panelType: string;
	title: string;
	component: PanelComponent;
	description?: string;
	defaultParams?: PanelParams;
	renderPolicy?: PanelRenderPolicy;
	origin?: 'golden' | 'user';
	/** Optional submenu group for the panel-creation menu (e.g. 'Module Editors'). */
	category?: string;
}

export interface UserPanelDefinition {
	title?: string;
	component: PanelComponent;
	description?: string;
	defaultParams?: PanelParams;
	renderPolicy?: PanelRenderPolicy;
	/** Optional submenu group for the panel-creation menu. */
	category?: string;
}

export type UserPanelDefinitionMap = Record<string, UserPanelDefinition | PanelComponent>;

export interface PanelSpawnPosition {
	referencePanelId?: string;
	direction?: PanelDockDirection;
}

export interface PanelSpawnRequest {
	panelType: string;
	required?: boolean;
	panelId?: string;
	title?: string;
	params?: PanelParams;
	renderPolicy?: PanelRenderPolicy;
	position?: PanelSpawnPosition;
	initialWidth?: number;
	initialHeight?: number;
	minimumWidth?: number;
	maximumWidth?: number;
	minimumHeight?: number;
	maximumHeight?: number;
	inactive?: boolean;
}

export interface PanelQuery {
	panelId?: string;
	panelType?: string;
}

export interface PanelHandle {
	readonly panelId: string;
	readonly panelType: string;
	getTitle: () => string;
	isActive: () => boolean;
	setActive: () => void;
	close: () => void;
	setTitle: (title: string) => void;
	updateParams: (params: PanelParams) => void;
	getParams: <T extends PanelParams = PanelParams>() => T;
}

export interface PanelController {
	accessPanel: (query: PanelQuery) => PanelHandle | null;
	getPanelById: (panelId: string) => PanelHandle | null;
	getPanelByType: (panelType: string) => PanelHandle | null;
	showPanel: (request: PanelSpawnRequest) => PanelHandle | null;
}

// Backward-compatible aliases while callers migrate off Dock-specific naming.
export type DockPanelParams = PanelParams;
export type DockPanelState = PanelState;
export type DockPanelProps = PanelProps;
export type DockPanelExports = PanelExports;
export type DockPanelComponent = PanelComponent;
