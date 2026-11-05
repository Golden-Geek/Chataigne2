import type { MediaEditResult } from '../../media/generated/MediaEditResult';
export function viewportResult(media: string, row = 0): MediaEditResult {
	return {
		kind: 'MatrixViewport',
		snapshot: {
			label: media,
			media_types: [],
			project: 'project-1',
			media,
			final_output: {
				reference: { media, channel: 'final' },
				label: 'Final',
				definition: {
					source: {
						kind: 'Matrix',
						output: {
							kind: 'Final',
							definition: {
								canvas: { width: 4, height: 2 },
								empty_cell: 'Transparent'
							}
						}
					},
					size: { kind: 'Automatic' },
					fit_mode: 'Fit'
				}
			},
			revision: 1,
			row_total: 100000,
			column_total: 100000,
			axis_total: 100000,
			axis_depth: 0,
			column_groups: [],
			requested_column_groups: [],
			row_start: row,
			axis_start: 0,
			rows: [],
			columns: [],
			cells: [],
			selected: null,
			playback: {
				request_id: null,
				layers: [],
				desired_column: null,
				launch_id: null,
				transition: { duration_ms: 250, curve: 'Linear' }
			}
		}
	};
}
