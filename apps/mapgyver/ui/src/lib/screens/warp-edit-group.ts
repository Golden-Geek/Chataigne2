import type { GestureEditGroup } from 'golden_ui/store/latest-edit-gesture';
import type { ScreenEdit } from './generated/ScreenEdit';

export type CornerLinkEdit = Extract<ScreenEdit, { kind: 'LinkScreenCorners' }>;

/** Flush the drag's optional authored link before ending its existing Undo group. */
export function warpEditGroup(
	group: GestureEditGroup,
	valid: () => boolean,
	takeLink: () => CornerLinkEdit | null,
	link: (edit: CornerLinkEdit) => Promise<unknown>
): GestureEditGroup {
	return {
		get active() {
			return group.active;
		},
		begin: () => group.begin(),
		async end() {
			const edit = takeLink();
			try {
				if (edit && group.active && valid()) await link(edit);
			} finally {
				await group.end();
			}
		}
	};
}
