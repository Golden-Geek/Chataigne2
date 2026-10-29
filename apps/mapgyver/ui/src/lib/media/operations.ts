import type { WorkbenchSession } from 'golden_ui';
import type { MediaEdit } from './generated/MediaEdit';
import { MEDIA_NAMESPACE, MEDIA_OPERATION } from './generated/constants';

/** Editors collect intents; backend operations own identity, defaults and validation. */
export const invokeMediaEdit = (
	session: Pick<WorkbenchSession, 'invokeAppOperation'>,
	edit: MediaEdit
) =>
	session.invokeAppOperation({
		kind: 'invokeAppOperation',
		namespace: MEDIA_NAMESPACE,
		operation: MEDIA_OPERATION,
		payload: edit
	});
