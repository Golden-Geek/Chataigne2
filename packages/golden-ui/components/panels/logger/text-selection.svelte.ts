import { untrack } from 'svelte';

const MAX_RETAINED_SELECTION_ROWS = 64;

/** Native selection pins only a bounded cohort; bulk history copy uses the logical entries. */
export function createLogTextSelection(list: () => HTMLElement | null, onSelection: () => void) {
	let keys = $state.raw<string[]>([]);
	$effect(() => {
		const element = list();
		if (!element) return;
		const update = () => {
			const selection = document.getSelection();
			const endpoints = [selection?.anchorNode, selection?.focusNode].map((node) =>
				(node instanceof Element ? node : node?.parentElement)?.closest<HTMLElement>(
					'[data-record-key]'
				)
			);
			if (
				!selection ||
				selection.isCollapsed ||
				!selection.rangeCount ||
				endpoints.some((row) => !row || !element.contains(row))
			) {
				keys = [];
				return;
			}
			const range = selection.getRangeAt(0);
			const retained = new Set(endpoints.map((row) => row!.dataset.recordKey!));
			for (const row of element.querySelectorAll<HTMLElement>('[data-record-key]')) {
				if (retained.size === MAX_RETAINED_SELECTION_ROWS) break;
				if (range.intersectsNode(row)) retained.add(row.dataset.recordKey!);
			}
			keys = [...retained];
			onSelection();
		};
		document.addEventListener('selectionchange', update);
		untrack(update);
		return () => document.removeEventListener('selectionchange', update);
	});
	return {
		get keys() {
			return keys;
		}
	};
}
