import type { GraphState } from '../../../../store/graph.svelte';
import type { WorkbenchSession } from '../../../../store/workbench.svelte';
import type { UiNodeDto } from '../../../../types';
import type { DashboardPlacement, DashboardLayoutKind } from '../dashboard-model';
import type { getDashboardWidgetOrderingState } from '../dashboard-actions';
import type { createDashboardInteractionChrome } from './interaction-chrome.svelte';
import type { createDashboardInteractionGeometry } from './interaction-geometry.svelte';

export interface DashboardOrderedLayoutContext {
	readonly session: WorkbenchSession | null;
	readonly graph: GraphState | null;
	readonly node: UiNodeDto;
	readonly parent: UiNodeDto | null;
	readonly placement: DashboardPlacement;
	readonly editMode: boolean;
	readonly widgetElement: HTMLElement | null;
	readonly parentLayoutKind: DashboardLayoutKind;
	readonly ordering: ReturnType<typeof getDashboardWidgetOrderingState>;
	readonly chrome: ReturnType<typeof createDashboardInteractionChrome>;
	readonly geometry: ReturnType<typeof createDashboardInteractionGeometry>;
	clearChrome(): void;
}
