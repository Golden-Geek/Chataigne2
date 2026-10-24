import { dev } from '$app/env';

export const ssr = false;
// Development needs response CSP nonces/headers; production still emits static hashed pages.
export const prerender = !dev;
