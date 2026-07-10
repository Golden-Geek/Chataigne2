import type { PanelApi, PanelParams } from './panel-types';

export const PERSISTED_PANEL_STATE_KEY = '__gc_panel_state';

const isRecord = (value: unknown): value is Record<string, unknown> =>
	typeof value === 'object' && value !== null && !Array.isArray(value);

const asPersistedState = (value: unknown): Record<string, unknown> => {
	if (!isRecord(value)) {
		return {};
	}
	return value;
};

export const readPanelPersistedState = <T extends object = Record<string, unknown>>(
	params: PanelParams | undefined
): Partial<T> => {
	if (!params) {
		return {};
	}
	return asPersistedState(params[PERSISTED_PANEL_STATE_KEY]) as Partial<T>;
};

export const writePanelPersistedState = (
	panelApi: PanelApi,
	nextState: Record<string, unknown>
): void => {
	const currentParams = panelApi.getParams<PanelParams>();
	const currentState = asPersistedState(currentParams[PERSISTED_PANEL_STATE_KEY]);
	const mergedState = {
		...currentState,
		...nextState
	};

	let didChange = false;
	for (const [key, value] of Object.entries(nextState)) {
		if (currentState[key] !== value) {
			didChange = true;
			break;
		}
	}

	if (!didChange) {
		return;
	}

	panelApi.updateParams({
		[PERSISTED_PANEL_STATE_KEY]: mergedState
	});
};
