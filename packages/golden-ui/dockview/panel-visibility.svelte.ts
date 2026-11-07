import { getContext } from 'svelte';
import type { PanelApi } from './panel-types';

export const panelVisibilityContext = Symbol('golden-panel-visibility');
export interface PanelVisibility {
	readonly visible: boolean;
}

/** One workbench-owned lifetime, inherited by every nested editor adapter. */
export function createPanelVisibility(api: Pick<PanelApi, 'observeVisibility'>) {
	let visible = $state(false);
	const dispose = api.observeVisibility((value) => {
		visible = value;
	});
	return {
		get visible() {
			return visible;
		},
		dispose
	};
}

/** Standalone/SSR content is visible; docked content inherits the actual owning tab. */
export const usePanelVisibility = (): PanelVisibility =>
	getContext<PanelVisibility | undefined>(panelVisibilityContext) ?? { visible: true };
