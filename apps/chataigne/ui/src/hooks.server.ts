import { dev } from '$app/env';
import type { Handle } from '@sveltejs/kit/hooks';
import { developmentContentSecurityPolicy } from 'golden_ui/host/content-security-policy';

export const handle: Handle = async ({ event, resolve }) => {
	const response = await resolve(event);
	if (dev && response.headers.get('content-type')?.includes('text/html')) {
		const policy = response.headers.get('content-security-policy');
		if (policy)
			response.headers.set(
				'content-security-policy',
				developmentContentSecurityPolicy(
					policy,
					event.url,
					Number(import.meta.env.VITE_GC_UI_BACKEND_PORT ?? 7010)
				)
			);
	}
	return response;
};
