const textEntry = 'input, textarea, select, [contenteditable="true"], [role="slider"]';
const control = `${textEntry}, button, a, [role="button"], [data-viewport-selection-target]`;
const closest = (target: EventTarget | null, selector: string) =>
	target instanceof Element ? target.closest(selector) : null;

interface PressSelection {
	disabled?: boolean;
	select(): void;
}

/** Select on down; reserve click for keyboard/accessibility activation, preserving native drag. */
export function selectOnPress(node: HTMLElement | SVGElement, initial: PressSelection) {
	let options = initial;
	const previous = node.getAttribute('data-viewport-selection-target');
	node.setAttribute('data-viewport-selection-target', '');
	const owns = (event: Event) => {
		const target = closest(event.target, control);
		return !options.disabled && !event.defaultPrevented && (!target || target === node);
	};
	function down(event: PointerEvent) {
		if (event.button === 0 && owns(event)) options.select();
	}
	function click(event: MouseEvent) {
		if (event.detail === 0 && owns(event)) options.select();
	}
	function key(event: KeyboardEvent) {
		if ((event.key === 'Enter' || event.key === ' ') && owns(event)) {
			event.preventDefault();
			event.stopPropagation();
			options.select();
		}
	}
	node.addEventListener('pointerdown', down as EventListener);
	node.addEventListener('click', click as EventListener);
	node.addEventListener('keydown', key as EventListener);
	return {
		update(next: PressSelection) {
			options = next;
		},
		destroy() {
			node.removeEventListener('pointerdown', down as EventListener);
			node.removeEventListener('click', click as EventListener);
			node.removeEventListener('keydown', key as EventListener);
			if (previous === null) node.removeAttribute('data-viewport-selection-target');
			else node.setAttribute('data-viewport-selection-target', previous);
		}
	};
}

/** Domain-neutral owner clearing; adapters retain tool cancellation and owner identities. */
export function clearViewportSelection(
	event: KeyboardEvent | PointerEvent,
	options: {
		disabled?: boolean;
		cancel?(): boolean;
		clear(): void;
	}
): boolean {
	if (options.disabled || event.defaultPrevented || event.ctrlKey || event.metaKey || event.altKey)
		return false;
	if (event.type === 'keydown') {
		if ((event as KeyboardEvent).key !== 'Escape' || closest(event.target, textEntry)) return false;
		event.preventDefault();
		event.stopPropagation();
		if (!options.cancel?.()) options.clear();
		return true;
	}
	if (
		event.type !== 'pointerdown' ||
		(event as PointerEvent).button !== 0 ||
		event.shiftKey ||
		closest(event.target, control)
	)
		return false;
	options.clear();
	return true;
}
