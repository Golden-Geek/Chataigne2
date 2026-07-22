import { describe, expect, it } from 'vitest';
import { shouldFetchParameterControlInfo } from 'golden_ui/components/panels/inspector/parameter-control-info.ts';

const refresh = (
	mode: 'manual' | 'contextLink',
	overrides: Partial<Parameters<typeof shouldFetchParameterControlInfo>[0]> = {}
) =>
	shouldFetchParameterControlInfo({
		mode,
		nodeChanged: false,
		enteredContextLink: false,
		openedMenu: false,
		finishedLoading: false,
		...overrides
	});

describe('parameter control-info loading', () => {
	it('does not fan out requests when manual parameter editors mount', () => {
		expect(refresh('manual', { nodeChanged: true })).toBe(false);
		expect(refresh('manual', { finishedLoading: true })).toBe(false);
	});

	it('loads on demand for the control menu', () => {
		expect(refresh('manual', { openedMenu: true })).toBe(true);
	});

	it('keeps context-link metadata current', () => {
		expect(refresh('contextLink', { nodeChanged: true })).toBe(true);
		expect(refresh('contextLink', { enteredContextLink: true })).toBe(true);
	});
});
