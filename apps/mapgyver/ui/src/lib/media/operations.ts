import type { WorkbenchSession } from 'golden_ui';
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
		'InspectManager',
		'InspectChannel',
		'InspectPictureFile',
		'InspectChannels',
		'InspectMediaPreview',
		'InspectChoices',
		'InspectCommandCatalog',
		'InspectCompositionEditor'
	].includes(edit.kind)
		? 'background'
		: 'user';
}

/** Editors collect intents; backend operations own identity, defaults and validation. */
export const invokeMediaEdit = async (
	session: Pick<WorkbenchSession, 'invokeAppOperation'>,
	edit: MediaEdit
) =>
	(await session.invokeAppOperation(
		{
			kind: 'invokeAppOperation',
			namespace: MEDIA_NAMESPACE,
			operation: MEDIA_OPERATION,
			payload: edit
		},
		undefined,
		{ activity: mediaOperationActivity(edit) }
	)) as MediaEditResult;
