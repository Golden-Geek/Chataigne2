import { mount } from 'svelte';
import { emit } from '@tauri-apps/api/event';
import { isTauri } from '@tauri-apps/api/core';
import { emptyQualificationReport } from './qualification';
import { QUALIFICATION_REPORT_EVENT } from './generated/constants';
import type { QualificationReport } from './generated/QualificationReport';

const qualification = new URLSearchParams(location.search).get('mode') === 'qualification';

function fail(reason: unknown) {
	const target = window as typeof window & {
		__previewSmoke?: {
			phase: string;
			trace: unknown[];
			error: string | null;
			maxRetirementMasks?: number;
		};
		__previewQualification?: QualificationReport;
	};
	const report = qualification
		? (target.__previewQualification ?? emptyQualificationReport())
		: (target.__previewSmoke ?? { phase: 'starting', trace: [], error: null });
	report.phase = 'failed';
	report.error = (
		reason instanceof Error ? (reason.stack ?? reason.message) : String(reason)
	).slice(0, 8192);
	if (qualification) target.__previewQualification = report as QualificationReport;
	else target.__previewSmoke = report as NonNullable<typeof target.__previewSmoke>;
	let detail = document.querySelector<HTMLPreElement>('[data-preview-bootstrap-error]');
	if (!detail) {
		detail = document.createElement('pre');
		detail.dataset.previewBootstrapError = '';
		detail.style.cssText =
			'margin:1em;padding:1em;background:#202329;color:white;white-space:pre-wrap';
		document.body.append(detail);
	}
	detail.textContent = report.error;
	if (isTauri())
		void emit(qualification ? QUALIFICATION_REPORT_EVENT : 'golden-preview-smoke', report).catch(
			() => {}
		);
}

window.addEventListener('error', (event) => fail(event.error ?? event.message));
window.addEventListener('unhandledrejection', (event) => fail(event.reason));

// Install diagnostic listeners before importing/mounting the actual Svelte fixture. Native
// protocol tests must report startup failures instead of waiting silently for a phase event.
async function start() {
	// Keep distinct import sites: each fixture must preload its own extracted stylesheet.
	if (qualification) {
		const { default: Fixture } = await import('./Qualification.svelte');
		mount(Fixture, { target: document.getElementById('app')! });
		return;
	}
	const { default: Fixture } = await import('./Fixture.svelte');
	mount(Fixture, { target: document.getElementById('app')! });
}
void start().catch(fail);
