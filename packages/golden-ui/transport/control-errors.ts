import type { UiControlErrorCode } from '../generated/rust_protocol/UiControlErrorCode';

/** A request failed before an authoritative acknowledgement became available. */
export class ControlRequestError extends Error {
	constructor(
		message: string,
		readonly code: UiControlErrorCode
	) {
		super(message);
		this.name = 'ControlRequestError';
	}
}

export function requestError(message: string, code: unknown): Error {
	switch (code) {
		case 'overloaded':
		case 'disconnected':
		case 'cancelled':
			return new ControlRequestError(message, code);
		default:
			return new Error(message);
	}
}

export function formatHttpError(context: string, response: Response, body: unknown): Error {
	const record =
		typeof body === 'object' && body !== null && !Array.isArray(body)
			? (body as Record<string, unknown>)
			: undefined;
	const suffix = typeof record?.error === 'string' ? `: ${record.error}` : '';
	return Object.assign(
		requestError(
			`${context} failed with ${response.status} ${response.statusText}${suffix}`,
			record?.code
		),
		{ status: response.status, statusText: response.statusText, body }
	);
}
