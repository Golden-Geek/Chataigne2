import type { UiAck, UiEditIntent } from '../../types';
import {
	userActionLabelForIntent,
	userActionLabelForIntents,
	type WorkbenchUserActionCoordinator
} from './action-activity.svelte';
import type { WorkbenchIntentPipeline, WorkbenchIntentProgressObserver } from './intent-pipeline';

export type AppliedIntentObserver = (acknowledgements: readonly UiAck[]) => void;
export function createIntentSender(
	pipeline: WorkbenchIntentPipeline,
	activity: Pick<WorkbenchUserActionCoordinator, 'run'>
) {
	const observer = (
		progress: WorkbenchIntentProgressObserver | undefined,
		applied: AppliedIntentObserver | undefined
	) =>
		applied
			? {
					...progress,
					applied: (acks: readonly UiAck[]) => {
						progress?.applied?.(acks);
						applied(acks);
					}
				}
			: progress;
	return {
		sendIntent(intent: UiEditIntent, applied?: AppliedIntentObserver): Promise<void> {
			const label = userActionLabelForIntent(intent);
			const dispatch = (progress?: WorkbenchIntentProgressObserver) =>
				pipeline.sendIntent(intent, observer(progress, applied));
			return label ? activity.run(label, dispatch) : dispatch();
		},
		sendIntents(intents: UiEditIntent[], applied?: AppliedIntentObserver): Promise<void> {
			const label = userActionLabelForIntents(intents);
			const dispatch = (progress?: WorkbenchIntentProgressObserver) =>
				pipeline.sendIntents(intents, observer(progress, applied));
			return label ? activity.run(label, dispatch) : dispatch();
		}
	};
}
