import { describe, it, expect } from 'vitest';
import {
	developmentContentSecurityPolicy,
	uiContentSecurityPolicy
} from '../content-security-policy.mjs';

describe('frontend content security policy', () => {
	it('preserves framework nonces and constrains dev backend/HMR to the selected host', () => {
		const policy = developmentContentSecurityPolicy(
			"script-src 'self' 'nonce-abc'; connect-src ws:; object-src 'none'",
			'http://192.168.20.44:5174/path',
			7041
		);
		expect(policy).toContain("script-src 'self' 'nonce-abc'");
		expect(policy).toContain(
			'http://192.168.20.44:7041 ws://192.168.20.44:7041 ws://192.168.20.44:5174'
		);
		expect(policy).not.toContain('connect-src ws:');
		expect(policy).not.toContain('http://localhost:');
		expect(policy).not.toContain('127.0.0.1');
		expect(uiContentSecurityPolicy.directives['script-src']).toEqual(['self']);
	});
	it('rejects opaque origins or invalid endpoint ports', () => {
		expect(() => developmentContentSecurityPolicy('', 'file:///ui', 7010)).toThrow();
		expect(() => developmentContentSecurityPolicy('', 'http://localhost:5173', NaN)).toThrow();
	});
});
