import type { UiAck, UiEditIntent } from '../../types';
import type { WorkbenchIntentPipeline } from './intent-pipeline';
import type { WorkbenchUserActionCoordinator } from './action-activity.svelte';

export type WorkbenchAppOperation = Extract<UiEditIntent, { kind: 'invokeAppOperation' }>;
export type WorkbenchAppOperationValue = NonNullable<UiAck['result']>['value'];

/** Retains the authoritative reply through the ordinary queue, history and activity path. */
export const invokeWorkbenchAppOperation = async (
	request: WorkbenchAppOperation,
	pipeline: Pick<WorkbenchIntentPipeline, 'sendIntent'>,
	activity: Pick<WorkbenchUserActionCoordinator, 'run'>
): Promise<WorkbenchAppOperationValue> => {
	let acknowledgements: readonly UiAck[] = [];
	await activity.run('Applying change', (progress) =>
		pipeline.sendIntent(request, {
			lifecycle: progress.lifecycle,
			applied(acks) {
				acknowledgements = acks;
				progress.applied?.(acks);
			}
		})
	);
	const ack = acknowledgements[0];
	if (acknowledgements.length !== 1 || !ack?.success || ack.status !== 'applied' || !ack.result) {
		throw new Error(ack?.error_message ?? 'App operation did not return one committed result.');
	}
	return ack.result.value;
};
