import { describe, expect, it } from 'vitest';
import { resolveRuntimeEndpoints } from '../runtimeEndpoints';

describe('resolveRuntimeEndpoints', () => {
	it('uses the complete browser-visible origin for a bundled host', () => {
		expect(
			resolveRuntimeEndpoints({ origin: 'http://192.168.20.44:7041' }, { development: false })
		).toEqual({
			httpBaseUrl: 'http://192.168.20.44:7041/api/ui',
			wsUrl: 'ws://192.168.20.44:7041/api/ui/ws'
		});
	});

	it('keeps the browser-visible hostname while targeting the development backend port', () => {
		expect(
			resolveRuntimeEndpoints({ origin: 'http://10.0.0.27:5173' }, { development: true })
		).toEqual({
			httpBaseUrl: 'http://10.0.0.27:7010/api/ui',
			wsUrl: 'ws://10.0.0.27:7010/api/ui/ws'
		});
	});

	it('uses secure transport schemes when the visible origin is HTTPS', () => {
		expect(
			resolveRuntimeEndpoints({ origin: 'https://chataigne.example.test' }, { development: false })
		).toEqual({
			httpBaseUrl: 'https://chataigne.example.test/api/ui',
			wsUrl: 'wss://chataigne.example.test/api/ui/ws'
		});
	});
});
