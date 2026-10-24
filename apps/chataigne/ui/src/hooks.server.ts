import { dev } from '$app/environment';
import { env } from '$env/dynamic/private';
import type { Handle } from '@sveltejs/kit';
import { developmentContentSecurityPolicy } from '../../../../packages/golden-ui/host/content-security-policy.mjs';

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
					Number(env.VITE_GC_UI_BACKEND_PORT ?? 7010)
				)
			);
	}
	return response;
};
