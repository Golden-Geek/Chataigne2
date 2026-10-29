import type { QualificationLayout } from './generated/QualificationLayout';
import type { QualificationCommand } from './generated/QualificationCommand';
import type { QualificationReport } from './generated/QualificationReport';

const u32 = (value: unknown): value is number =>
	typeof value === 'number' && Number.isInteger(value) && value >= 0 && value <= 0xffffffff;

export function qualificationLayout(value: unknown): QualificationLayout {
	const layout = value as QualificationLayout | null;
	if (!layout || !u32(layout.revision) || !Array.isArray(layout.views) || layout.views.length !== 2)
		throw new Error('Fixture layout requires a revision and exactly two views');
	for (const view of layout.views) {
		if (
			!view ||
			typeof view.source !== 'string' ||
			view.source.length === 0 ||
			view.source.includes('\0') ||
			new TextEncoder().encode(view.source).length > 255 ||
			![view.cssWidth, view.cssHeight].every(
				(size) => typeof size === 'number' && Number.isFinite(size) && size >= 1 && size <= 16384
			)
		)
			throw new Error('Fixture source keys or host CSS dimensions are invalid');
	}
	return structuredClone(layout);
}

export function qualificationCommand(value: unknown): QualificationCommand {
	const command = value as QualificationCommand | null;
	if (
		!command ||
		!u32(command.id) ||
		!['ready', 'hide', 'show', 'disconnect'].includes(command.action)
	)
		throw new Error('Fixture command requires an id and a supported action');
	return { id: command.id, action: command.action };
}

export function queryQualificationLayout(): QualificationLayout | null {
	const query = new URLSearchParams(location.search);
	const fields = ['source1', 'source2', 'width1', 'height1', 'width2', 'height2'];
	if (!fields.some((field) => query.has(field))) return null;
	return qualificationLayout({
		revision: 0,
		views: [1, 2].map((index) => ({
			source: query.get(`source${index}`),
			cssWidth: Number(query.get(`width${index}`)),
			cssHeight: Number(query.get(`height${index}`))
		}))
	});
}

export function emptyQualificationReport(): QualificationReport {
	return {
		sequence: 0,
		commandId: null,
		phase: 'listening',
		layout: null,
		installed: false,
		visibleViews: 0,
		views: [],
		retirementMasks: 0,
		maxRetirementMasks: 0,
		overlayClicks: [0, 0],
		error: null
	};
}
