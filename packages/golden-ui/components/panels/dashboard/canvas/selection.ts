import type { SelectionMode } from '../../../../store/workbench.svelte';
import { focusClosestPageViewport, selectionModeFromEvent } from './dom';
import { clearViewportSelection } from '../../../viewport/selection';

const isTextEntry = (target: EventTarget | null) =>
	target instanceof Element &&
	Boolean(target.closest('input, textarea, select, [contenteditable="true"]'));

export function dashboardWidgetPointerDown(
	event: PointerEvent,
	context: {
		editing: boolean;
		selected: boolean;
		select(mode: SelectionMode): void;
		move?: (event: PointerEvent) => void;
	}
) {
	focusClosestPageViewport(event.currentTarget);
	if (!context.editing || event.button !== 0) return;
	event.stopPropagation();
	const mode = selectionModeFromEvent(event);
	if (mode !== 'REPLACE') {
		context.select(mode);
		return;
	}
	if (!context.selected || !context.move) context.select('REPLACE');
	context.move?.(event);
}

export function dashboardWidgetKeyDown(
	event: KeyboardEvent,
	editing: boolean,
	select: (mode: SelectionMode) => void
) {
	if (
		!editing ||
		event.defaultPrevented ||
		isTextEntry(event.target) ||
		(event.key !== 'Enter' && event.key !== ' ')
	)
		return;
	event.preventDefault();
	event.stopPropagation();
	select(selectionModeFromEvent(event));
}

export function dashboardPageKeyDown(
	event: KeyboardEvent,
	context: {
		editing: boolean;
		cancel(): boolean;
		selectOwner(): void;
	}
) {
	clearViewportSelection(event, {
		disabled: !context.editing,
		cancel: context.cancel,
		clear: context.selectOwner
	});
}
