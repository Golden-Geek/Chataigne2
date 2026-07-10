import { appState } from 'golden_ui/store/workbench.svelte';

import type { FormulaPreviewModeDto } from '../generated';

export const STATE_MACHINE_RUNTIME_PREVIEW_INTEREST_TOPIC =
	'chataigne.state_machine.runtime_preview_interest';

let lastSession = appState.session;
const signatures = new Map<string, string>();

export const setRuntimePreviewInterest = (
	viewId: string,
	mode: FormulaPreviewModeDto | null
): void => {
	const session = appState.session;
	if (session !== lastSession) {
		lastSession = session;
		signatures.clear();
	}
	const signature = mode === null ? '' : JSON.stringify(mode);
	if (signatures.get(viewId) === signature) return;
	signatures.set(viewId, signature);
	if (!session) return;
	void session
		.sendIntent({
			kind: 'setRuntimeViewInterest',
			view_id: viewId,
			topic: STATE_MACHINE_RUNTIME_PREVIEW_INTEREST_TOPIC,
			payload: mode
		})
		.catch((error: unknown) => {
			console.error('failed to update runtime preview interest', error);
			signatures.delete(viewId);
		});
};
