import { describe, expect, it } from 'vitest';
import { ControlRequestError, formatHttpError, requestError } from '../control-errors';

describe('control request failures', () => {
	it.each(['overloaded', 'disconnected', 'cancelled'])('retains the %s category', (code) => {
		const error = requestError('request could not complete', code);
		expect(error).toBeInstanceOf(ControlRequestError);
		expect((error as ControlRequestError).code).toBe(code);
		expect(error.message).toBe('request could not complete');
	});

	it('keeps ordinary protocol failures distinct from actor failures', () => {
		expect(requestError('invalid payload', undefined)).not.toBeInstanceOf(ControlRequestError);
		expect(requestError('future code', 'unknown')).not.toBeInstanceOf(ControlRequestError);
	});

	it('retains HTTP status and typed failure without manufacturing an acknowledgement', () => {
		const body = { error: 'queue is full', code: 'overloaded' };
		const error = formatHttpError(
			'intent',
			new Response(null, { status: 503, statusText: 'Service Unavailable' }),
			body
		);
		expect(error).toBeInstanceOf(ControlRequestError);
		expect(error).toMatchObject({ code: 'overloaded', status: 503, body });
		expect(error.message).toContain('queue is full');
	});
});
