import type { WorkbenchSession } from 'golden_ui';
import type { ScreenEdit } from './generated/ScreenEdit';
import type { ScreenEditResult } from './generated/ScreenEditResult';
import { SCREEN_NAMESPACE, SCREEN_OPERATION } from './generated/constants.ts';

/** Selection/history refreshes are inspections, including the per-panel editor query. */
export const screenOperationActivity = (edit: ScreenEdit): 'background' | 'user' =>
	['InspectRow', 'InspectManager', 'InspectScreen', 'InspectSurface', 'InspectEditor'].includes(
		edit.kind
	)
		? 'background'
		: 'user';

/** Backend operations own Screen/Surface identities, defaults, validation and history. */
export const invokeScreenEdit = async (
	session: Pick<WorkbenchSession, 'invokeAppOperation'>,
	edit: ScreenEdit
) =>
	(await session.invokeAppOperation(
		{
			kind: 'invokeAppOperation',
			namespace: SCREEN_NAMESPACE,
			operation: SCREEN_OPERATION,
			payload: edit
		},
		undefined,
		{
			activity: screenOperationActivity(edit)
		}
	)) as ScreenEditResult;
