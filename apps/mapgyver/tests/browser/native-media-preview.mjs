// Real bundled WebView2 controls and completion-based native preview installation.
// Independent output pixels remain a separate M04/8 acceptance requirement.
import assert from 'node:assert/strict';
import { openNativeEditor } from './native-editor.mjs';
import { verifyMediaPreviewUi } from './media-preview.mjs';

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
	await verifyMediaPreviewUi({
		base: native.base,
		page: native.page,
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
