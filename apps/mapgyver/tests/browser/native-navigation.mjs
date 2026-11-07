import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdir } from 'node:fs/promises';
import path from 'node:path';
import { openNativeEditor } from './native-editor.mjs';
import { observeNativeAdapter } from './native-adapter.mjs';
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
import { verifyNativeMediaFit } from './native-media-fit.mjs';
import { verifyNativeCompactChannels } from './native-compact-channels.mjs';
import { verifyPictureFiles } from './picture-files.mjs';
import { verifyNativePreviewCadence } from './native-preview-cadence.mjs';
import { verifyNativeSenderSelection } from './native-sender-selection.mjs';
import { verifyMatrixViewportUi } from './matrix-viewport.mjs';
import { verifyMatrixParametersUi } from './matrix-parameters.mjs';
import { verifyCompositionCreation } from './composition-creation.mjs';
import { verifyMatrixCellPreviews } from './matrix-cell-previews.mjs';
import { verifyMatrixPreviewAdmission } from './matrix-preview-admission.mjs';
import { verifyMatrixOutputExposure } from './matrix-output-exposure.mjs';
import { verifyMatrixSparsePreviews } from './matrix-sparse-previews.mjs';
import { verifyWorkbenchIdentity } from './workbench-identity.mjs';
import { verifyScreenIdentity } from './screen-identity.mjs';
import { verifyNativeSurfaceEnabled } from './native-surface-enabled.mjs';
import { verifyApplicationReentry } from '../headless/reentry.mjs';
import { verifyNativeApplicationEndpoint } from './native-application-endpoint.mjs';
import { verifyColumnGroupsUi } from './column-groups.mjs';
import { verifyColumnGroupRuntime } from './column-group-runtime.mjs';
import { verifyAdaptiveMatrixGrid } from './matrix-adaptive-grid.mjs';
import { verifyMatrixGridUi } from './matrix-grid.mjs';
import { verifyInspectorContentBinding } from './inspector-content-binding.mjs';
import { verifyPreviewContinuity } from './preview-continuity.mjs';
import { verifyScreenBindings } from './screen-bindings.mjs';
import { verifyMatrixBindings } from './matrix-bindings.mjs';
import { verifySpatialBindings } from './spatial-bindings.mjs';
import { verifyMatrixLayerGeometry } from './matrix-layer-geometry.mjs';
import { verifyMatrixCellLayout } from './matrix-cell-layout.mjs';
import { verifyMatrixCellGeometry } from './matrix-cell-geometry.mjs';
import { verifySpatialAcceptance } from './spatial-acceptance.mjs';
import { verifyMenuIconsInBothHosts } from './menu-icons.mjs';

const gpu = process.argv.find((value) => value.startsWith('--gpu='))?.slice('--gpu='.length);
assert(!gpu || /^dxgi:[0-9a-f]{8}:[0-9a-f]{8}$/.test(gpu), 'Exact GPU LUID required');
assert(!gpu || !process.env.MAPGYVER_TEST_APP_DATA, 'Pin GPUs only in a fresh owned profile');
const observer = path.resolve('target/debug/native_qualification-m1110.exe');
const native = await openNativeEditor({
	appDataDirectory: process.env.MAPGYVER_TEST_APP_DATA,
	prepareProfile: gpu
		? async (root) => {
				const preferences = path.join(root, 'MapGyver');
				await mkdir(preferences, { recursive: true });
				execFileSync(
					observer,
					[
						'--role',
						'gpu-preferences',
						'--file',
						path.join(preferences, 'preferences.json'),
						'--selected',
						gpu
					],
					{ windowsHide: true }
				);
			}
		: undefined
});
try {
	console.log(JSON.stringify({ flow: process.argv.slice(2), nativePid: native.app.pid }));
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
			['--menu-icons', verifyMenuIconsInBothHosts],
			['--spatial-acceptance', verifySpatialAcceptance],
			['--matrix-cell-geometry', verifyMatrixCellGeometry],
			['--matrix-cell-layout', verifyMatrixCellLayout],
			['--matrix-layer-geometry', verifyMatrixLayerGeometry],
			['--content-binding', verifyInspectorContentBinding],
			['--preview-bindings', verifyPreviewContinuity],
			['--screen-bindings', verifyScreenBindings],
			['--matrix-bindings', verifyMatrixBindings],
			['--spatial-bindings', verifySpatialBindings],
			[
				'--matrix-grid',
				async (options) => {
					await verifyAdaptiveMatrixGrid(options);
					await verifyMatrixGridUi(options);
					await verifyMatrixViewportUi(options);
					await verifyColumnGroupsUi(options);
					await verifyMatrixSparsePreviews(options);
				}
			],
			['--workbench-identity', verifyWorkbenchIdentity],
			['--screen-identity', verifyScreenIdentity],
			['--surface-enabled', verifyNativeSurfaceEnabled],
			['--application-reentry', verifyApplicationReentry],
			['--application-endpoint', verifyNativeApplicationEndpoint],
			['--column-groups', verifyColumnGroupsUi],
			['--column-group-runtime', verifyColumnGroupRuntime],
			[
				'--matrix-native-acceptance',
				async (options) => {
					await verifyMatrixPreviewAdmission(options);
					await verifyMatrixCellPreviews(options);
					await verifyMatrixSparsePreviews(options);
				}
			],
			['--matrix-sparse-previews', verifyMatrixSparsePreviews],
			['--matrix-output-exposure', verifyMatrixOutputExposure],
			['--matrix-preview-admission', verifyMatrixPreviewAdmission],
			['--matrix-cell-previews', verifyMatrixCellPreviews],
			['--matrix-viewport', verifyMatrixViewportUi],
			['--matrix-parameters', verifyMatrixParametersUi],
			['--composition-creation', verifyCompositionCreation],
			[
				'--canvas-remaining',
				async (options) => {
					await verifyScreenKeyboard(options);
					await verifyMatrixViewportUi(options);
					await verifyScreenSelection(options);
				}
			],
			[
				'--canvas-acceptance',
				async (options) => {
					await verifyViewportNavigation(options);
					await verifyCompositionHandles(options);
					await verifyScreenKeyboard(options);
					await verifyMatrixViewportUi(options);
					await verifyScreenSelection(options);
				}
			],
			['--sender-selection', verifyNativeSenderSelection],
			[
				'--inspector-acceptance',
				async (options) => {
					await verifyPictureFiles(options);
					await verifyNativeCompactChannels(options);
				}
			],
			['--preview-cadence', verifyNativePreviewCadence],
			['--channel-file', verifyPictureFiles],
			['--compact-channels', verifyNativeCompactChannels],
			['--media-fit', verifyNativeMediaFit],
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
		nativeLog: native.log,
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
	if (gpu) {
		const hardware = JSON.parse(
			execFileSync(observer, ['--role', 'hardware'], {
				encoding: 'utf8',
				windowsHide: true
			})
		);
		const adapter = await observeNativeAdapter(native.page, hardware);
		assert.equal(adapter.key, gpu);
		console.log(JSON.stringify({ nativeAdapter: adapter }));
	}
} finally {
	await native.close();
}
