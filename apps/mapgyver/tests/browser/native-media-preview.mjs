// Real bundled WebView2 controls and completion-based native preview installation.
// --continuity adds eight-view concurrency, reconnect/New/Load and independent output pixels.
// Build MapGyver/native_qualification with native-qualification; pixel checks need Python/Pillow.
import assert from 'node:assert/strict';
import { openNativeEditor } from './native-editor.mjs';
import { verifyMediaPreviewUi } from './media-preview.mjs';
import { verifyPreviewContinuity } from './preview-continuity.mjs';
import { createPreviewOutputObserver } from './preview-output-observer.mjs';
import path from 'node:path';

const native = await openNativeEditor();
let output;
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
	const flow = process.argv.includes('--continuity')
		? verifyPreviewContinuity
		: verifyMediaPreviewUi;
	if (process.argv.includes('--continuity'))
		output = await createPreviewOutputObserver(
			post,
			path.resolve(`target/mapgyver-preview-acceptance-${Date.now()}`),
			native.page
		);
	await flow({
		base: native.base,
		page: native.page,
		post,
		directory: native.directory,
		observe: output?.observe,
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
	if (output)
		console.log(
			JSON.stringify({ independentOutput: await output.finish(), pixels: output.observations })
		);
} finally {
	try {
		await output?.finish();
	} finally {
		await native.close();
	}
}
