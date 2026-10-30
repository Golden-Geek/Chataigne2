import type { WorkbenchSession } from 'golden_ui';
import type { MediaEdit } from './generated/MediaEdit';
import type { MediaEditResult } from './generated/MediaEditResult';
import { MEDIA_NAMESPACE, MEDIA_OPERATION } from './generated/constants';

/** Editors collect intents; backend operations own identity, defaults and validation. */
export const invokeMediaEdit = async (
	session: Pick<WorkbenchSession, 'invokeAppOperation'>,
	edit: MediaEdit
) =>
	(await session.invokeAppOperation({
		kind: 'invokeAppOperation',
		namespace: MEDIA_NAMESPACE,
		operation: MEDIA_OPERATION,
		payload: edit
	})) as MediaEditResult;
