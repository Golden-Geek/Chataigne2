import { untrack } from 'svelte';
import type { PanelApi } from 'golden_ui';

/** The displayed projection owns the name; selection and lock policy stay with its editor. */
export function followPanelTitle(
	api: () => PanelApi,
	kind: () => string,
	label: () => string | null | undefined
) {
	let previous: { api: PanelApi; title: string } | null = null;
	$effect(() => {
		const current = api(),
			name = label();
		const base = kind();
		const title = name ? `${base}: ${name}` : base;
		untrack(() => {
			if (previous?.api === current && previous.title === title) return;
			current.setTitle(title);
			previous = { api: current, title };
		});
	});
}
