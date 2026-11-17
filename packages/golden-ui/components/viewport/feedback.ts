/** Paint timing shared by browser controls and native overlay presentation. Never ease geometry. */
export const VIEWPORT_FEEDBACK_MS = 120;

export function viewportHandleAppearance(selected = false, ghost = false) {
	return {
		fill: ghost ? '00000000' : selected ? 'a3bfffd9' : 'dceaffb3',
		stroke: ghost ? 'ffe19bcc' : selected ? 'dddddd' : '467bcc',
		halo: '1c273dcc',
		radius: 0.375,
		width: 0.1
	};
}
