import type { UiAck, UiEditIntent } from '../../types';
import type { WorkbenchIntentPipeline, WorkbenchIntentProgressObserver } from './intent-pipeline';
import type { WorkbenchUserActionCoordinator } from './action-activity.svelte';
import type { AppliedIntentObserver } from './intent-sender';

export type WorkbenchAppOperation = Extract<UiEditIntent, { kind: 'invokeAppOperation' }>;
export type WorkbenchAppOperationValue = NonNullable<UiAck['result']>['value'];
export interface WorkbenchAppOperationOptions {
	/** Background inspections retain queue/receipt fencing without presenting a user action. */
	activity?: 'user' | 'background';
}

/** Retains the authoritative reply through the ordinary queue, history and activity path. */
export const invokeWorkbenchAppOperation = async (
	request: WorkbenchAppOperation,
	pipeline: Pick<WorkbenchIntentPipeline, 'sendIntent'>,
	activity: Pick<WorkbenchUserActionCoordinator, 'run'>,
	applied?: AppliedIntentObserver,
	options?: WorkbenchAppOperationOptions
): Promise<WorkbenchAppOperationValue> => {
	let acknowledgements: readonly UiAck[] = [];
	const dispatch = (progress?: WorkbenchIntentProgressObserver) =>
		pipeline.sendIntent(request, {
			lifecycle: progress?.lifecycle,
			applied(acks) {
				acknowledgements = acks;
				progress?.applied?.(acks);
				applied?.(acks);
			}
		});
	if (options?.activity === 'background') await dispatch();
	else await activity.run('Applying change', dispatch);
	const ack = acknowledgements[0];
	if (acknowledgements.length !== 1 || !ack?.success || ack.status !== 'applied' || !ack.result) {
		throw new Error(ack?.error_message ?? 'App operation did not return one committed result.');
	}
	return ack.result.value;
};
