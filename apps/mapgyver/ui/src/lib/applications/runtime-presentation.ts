import type { ApplicationRuntimeStatus } from '../media/generated/ApplicationRuntimeStatus';
import type { ApplicationSnapshot } from '../media/generated/ApplicationSnapshot';
import type { ApplicationServiceStatus } from '../media/generated/ApplicationServiceStatus';

export function serviceLabel(status: ApplicationServiceStatus) {
	switch (status.kind) {
		case 'Inactive':
			return 'Not attached';
		case 'Starting':
			return 'Starting';
		case 'Running':
			return 'Running';
		case 'Retrying':
			return `Retrying: ${status.message}`;
		case 'Halted':
			return `Halted: ${status.message}`;
	}
}

/** Identity/revision fencing is presentation; configuration policy comes from the worker. */
export function configurationLabel(
	status: ApplicationRuntimeStatus,
	selected: ApplicationSnapshot | null
) {
	const consumer =
		selected && status.consumers.find((consumer) => consumer.application === selected.id);
	const configuration =
		status.configuration === 'NotRunning'
			? status.configuration
			: (consumer?.configuration ?? status.configuration);
	if (
		selected &&
		(configuration === 'Current' || configuration === 'Pending') &&
		(!consumer ||
			consumer.definition_revision !== selected.revision ||
			consumer.definition_instance !== selected.definition_instance)
	)
		return 'Saved settings awaiting runtime reconciliation';
	switch (configuration) {
		case 'NotRunning':
			return 'No configuration in use';
		case 'Current':
			return 'Using reconciled saved settings';
		case 'Pending':
			return 'Saved changes pending; previous configuration still in use';
		case 'Retained':
			return 'Previous configuration retained until final release';
	}
}
