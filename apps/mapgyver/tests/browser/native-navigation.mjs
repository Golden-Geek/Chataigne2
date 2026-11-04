import assert from 'node:assert/strict';
import { openNativeEditor } from './native-editor.mjs';
import { verifyViewportNavigation } from './viewport-navigation.mjs';
import { verifyScreenSelection } from './screen-selection.mjs';
import { verifyScreenKeyboard } from './screen-keyboard.mjs';
import { verifyScreenTranslation } from './screen-translation.mjs';
import { verifyScreenFit } from './screen-fit.mjs';
import { verifyRuntimeFooter } from './runtime-footer.mjs';
import { verifyScreenDetails } from './screen-details.mjs';
import { verifySurfaceSourceUi } from './surface-source.mjs';
import { verifyCompositionHandles } from './composition-handles.mjs';
import { verifyCompositionUi } from './composition.mjs';
import { verifyMatrixFeedback } from './matrix-feedback.mjs';
import { verifyInspectorStability } from './inspector-stability.mjs';
import { verifyPreviewSelectionStability } from './preview-selection-stability.mjs';
import { verifyNativePreviewContinuity } from './native-preview-continuity.mjs';
import { verifyNativeInputStability } from './native-input-stability.mjs';
import { verifyNativeProjectTransitions } from './native-project-transitions.mjs';
import { verifyNativeCapacity } from './native-capacity.mjs';
import { verifyNativeTimeControls } from './native-time-controls.mjs';

const native = await openNativeEditor();
try {
	let token;
	async function post(route, body) {
		const response = await fetch(`${native.base}${route}`, {
			method: 'POST',
			signal: AbortSignal.timeout(5000),
			headers: {
				'Content-Type': 'application/json',
				...(token ? { Authorization: `Bearer ${token}` } : {})
			},
			body: JSON.stringify(body)
		});
		const value = await response.json();
		assert(response.ok, JSON.stringify(value));
		return value;
	}
	token = (await post('/api/ui/session', {})).token;
	const flow =
		[
			['--time-controls', verifyNativeTimeControls],
			['--capacity', verifyNativeCapacity],
			['--project-transitions', verifyNativeProjectTransitions],
			['--input-stability', verifyNativeInputStability],
			['--preview-continuity', verifyNativePreviewContinuity],
			['--preview-selection-stability', verifyPreviewSelectionStability],
			['--inspector-stability', verifyInspectorStability],
			['--matrix-feedback', verifyMatrixFeedback],
			['--composition-panels', verifyCompositionUi],
			['--composition', verifyCompositionHandles],
			[
				'--source',
				async (options) => {
					await options.edit({
						kind: 'CreateMedia',
						media_kind: 'SolidColor',
						label: 'Native source crop'
					});
					const inspectMedia = async () =>
						(await options.edit({ kind: 'InspectManager' })).snapshot;
					await verifySurfaceSourceUi({ ...options, media: await inspectMedia(), inspectMedia });
				}
			],
			['--details', verifyScreenDetails],
			['--footer', verifyRuntimeFooter],
			['--fit', verifyScreenFit],
			['--translation', verifyScreenTranslation],
			['--keyboard', verifyScreenKeyboard],
			['--selection', verifyScreenSelection]
		].find(([flag]) => process.argv.includes(flag))?.[1] ?? verifyViewportNavigation;
	await flow({
		nativePid: native.app.pid,
		base: native.base,
		page: native.page,
		post,
		directory: native.directory,
		edit: async (payload) => {
			const ack = await post('/api/ui/intent', {
				kind: 'invokeAppOperation',
				namespace: 'mapgyver.media',
				operation: 'edit',
				payload
			});
			assert(ack.success, JSON.stringify(ack));
			return ack.result.value;
		}
	});
} finally {
	await native.close();
}
