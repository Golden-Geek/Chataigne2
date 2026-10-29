import { mount } from 'svelte';
import { emit } from '@tauri-apps/api/event';
import { isTauri } from '@tauri-apps/api/core';

function fail(reason: unknown) {
	const target = window as typeof window & {
		__previewSmoke?: {
			phase: string;
			trace: unknown[];
			error: string | null;
			maxRetirementMasks?: number;
		};
	};
	const report = target.__previewSmoke ?? { phase: 'starting', trace: [], error: null };
	report.phase = 'failed';
	report.error = (
		reason instanceof Error ? (reason.stack ?? reason.message) : String(reason)
	).slice(0, 8192);
	target.__previewSmoke = report;
	let detail = document.querySelector<HTMLPreElement>('[data-preview-bootstrap-error]');
	if (!detail) {
		detail = document.createElement('pre');
		detail.dataset.previewBootstrapError = '';
		detail.style.cssText =
			'margin:1em;padding:1em;background:#202329;color:white;white-space:pre-wrap';
		document.body.append(detail);
	}
	detail.textContent = report.error;
	if (isTauri()) void emit('golden-preview-smoke', report).catch(() => {});
}

window.addEventListener('error', (event) => fail(event.error ?? event.message));
window.addEventListener('unhandledrejection', (event) => fail(event.reason));

// Install diagnostic listeners before importing/mounting the actual Svelte fixture. Native
// protocol tests must report startup failures instead of waiting silently for a phase event.
void import('./Fixture.svelte')
	.then(({ default: Fixture }) => {
		try {
			mount(Fixture, { target: document.getElementById('app')! });
		} catch (error) {
			fail(error);
		}
	})
	.catch(fail);
