import type { WorkbenchSession } from 'golden_ui';
import type { AppliedIntentObserver } from 'golden_ui/store/session/intent-sender';
import type { MediaEdit } from './generated/MediaEdit';
import type { MediaEditResult } from './generated/MediaEditResult';
import { MEDIA_NAMESPACE, MEDIA_OPERATION } from './generated/constants.ts';

/** Inspection cancellation during resync is background feedback, not a failed user edit. */
export function mediaOperationActivity(edit: MediaEdit): 'background' | 'user' {
	if (edit.kind === 'Matrix')
		return ['Inspect', 'InspectPlayback', 'InspectTargets', 'InspectViewport'].includes(
			edit.edit.kind
		)
			? 'background'
			: 'user';
	if (edit.kind === 'Applications')
		return ['Inspect', 'InspectCallbacks', 'InspectRuntime'].includes(edit.edit.kind)
			? 'background'
			: 'user';
	if (edit.kind === 'Composition') return edit.edit.kind === 'Inspect' ? 'background' : 'user';
	return [
		'InspectTargetPicker',
		'InspectManager',
		'InspectChannel',
		'InspectPictureFile',
		'InspectChannels',
		'InspectMediaPreview',
		'InspectChoices',
		'InspectCommandCatalog',
		'InspectCompositionEditor',
		'InspectMatrixEditor',
		'InspectMatrixCells',
		'InspectSpatialEditor'
	].includes(edit.kind)
		? 'background'
		: 'user';
}

/** Editors collect intents; backend operations own identity, defaults and validation. */
export const invokeMediaEdit = async (
	session: Pick<WorkbenchSession, 'invokeAppOperation'>,
	edit: MediaEdit,
	applied?: AppliedIntentObserver
) =>
	(await session.invokeAppOperation(
		{
			kind: 'invokeAppOperation',
			namespace: MEDIA_NAMESPACE,
			operation: MEDIA_OPERATION,
			payload: edit
		},
		applied,
		{ activity: mediaOperationActivity(edit) }
	)) as MediaEditResult;
