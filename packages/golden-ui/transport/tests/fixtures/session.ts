import type { UiSessionProvider } from '../../session';
import { UI_PROTOCOL_VERSION } from '../../../generated/rust_protocol/protocol-version';

export const testSessionProvider: UiSessionProvider = {
	async credentials() {
		return {
			token: 'a'.repeat(43),
			capabilities: {
				observe: true,
				author: true,
				project_files: true,
				scripts: true
			},
			expires_in_ms: 3600000,
			protocol_version: UI_PROTOCOL_VERSION,
			websocket_protocols: ['golden-ui', `gc-session.${'a'.repeat(43)}`]
		};
	},
	invalidate() {}
};
