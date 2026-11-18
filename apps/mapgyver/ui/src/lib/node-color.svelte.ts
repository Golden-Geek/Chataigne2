import { appState } from 'golden_ui';

/** Indexed metadata only: no extra document read, preview lease or source renderer. */
export function customNodeColor(uuid: string): string | null {
	const color = appState.session?.graph.state.nodesById.getByUuid(uuid)?.meta.presentation?.color;
	return color
		? [color.r, color.g, color.b, color.a]
				.map((value) =>
					Math.round(Math.max(0, Math.min(1, value)) * 255)
						.toString(16)
						.padStart(2, '0')
				)
				.join('')
		: null;
}
