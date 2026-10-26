/** Dialog focus order uses the bounded, currently mounted controls, including the roving tree row. */
export function movePickerFocus(panel: HTMLElement | null, backwards: boolean): boolean {
	if (!panel) return false;
	const targets = Array.from(
		panel.querySelectorAll<HTMLElement>('button, input, select, textarea, [tabindex]')
	).filter(
		(element) =>
			element.tabIndex >= 0 &&
			!element.matches(':disabled') &&
			!element.closest('[inert]') &&
			element.getClientRects().length > 0 &&
			getComputedStyle(element).visibility !== 'hidden'
	);
	if (!targets.length) {
		panel.focus();
		return true;
	}
	const current = targets.indexOf(document.activeElement as HTMLElement);
	const next =
		current < 0
			? backwards
				? targets.length - 1
				: 0
			: (current + (backwards ? -1 : 1) + targets.length) % targets.length;
	targets[next].focus();
	return document.activeElement === targets[next];
}
