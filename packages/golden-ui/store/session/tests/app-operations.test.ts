import { describe, expect, it } from 'vitest';
import { invokeWorkbenchAppOperation, type WorkbenchAppOperation } from '../app-operations';
import type { UiAck } from '../../../types';
import type { WorkbenchUserActionCoordinator } from '../action-activity.svelte';

const request: WorkbenchAppOperation = {
	kind: 'invokeAppOperation',
	namespace: 'fixture',
	operation: 'paste',
	payload: {}
};
const activity: Pick<WorkbenchUserActionCoordinator, 'run'> = {
	run: async (_label, dispatch) => dispatch({})
};
const reply = (value: NonNullable<UiAck['result']>['value']): UiAck => ({
	success: true,
	status: 'applied',
	result: { value }
});

describe('workbench app operation results', () => {
	it('forwards the canonical applied receipt to a caller projection observer', async () => {
		const acks = [reply({ created: 123 })];
		let observed: readonly UiAck[] | undefined;
		await invokeWorkbenchAppOperation(
			request,
			{
				async sendIntent(_intent, progress) {
					progress?.applied?.(acks);
				}
			},
			activity,
			(value) => {
				observed = value;
			}
		);
		expect(observed).toBe(acks);
	});
	it.each([null, { source: 99, created: 123, uuid: 'exact' }])(
		'returns the backend value through ordinary activity without graph discovery (%j)',
		async (value) => {
			const result = await invokeWorkbenchAppOperation(
				request,
				{
					async sendIntent(intent, progress) {
						expect(intent).toBe(request);
						progress?.applied?.([reply(value)]);
					}
				},
				activity
			);
			expect(result).toEqual(value);
		}
	);
	it('retains an exact result while presentation completion is delayed', async () => {
		let release!: () => void;
		const paint = new Promise<void>((resolve) => {
			release = resolve;
		});
		const value = { source: 99, created: 123 };
		const result = invokeWorkbenchAppOperation(
			request,
			{
				async sendIntent(_intent, progress) {
					progress?.applied?.([reply(value)]);
				}
			},
			{
				async run(_label, dispatch) {
					await dispatch({});
					await paint;
				}
			}
		);
		release();
		expect(await result).toEqual(value);
	});
	it.each(['missing', 'staged', 'rejected', 'multiple'])(
		'rejects %s completion instead of inventing an identity',
		async (kind) => {
			const ack: UiAck = kind === 'missing' ? { success: true, status: 'applied' } : reply(123);
			if (kind === 'staged') ack.status = 'staged';
			if (kind === 'rejected') ack.success = false;
			await expect(
				invokeWorkbenchAppOperation(
					request,
					{
						async sendIntent(_intent, progress) {
							progress?.applied?.(kind === 'multiple' ? [ack, ack] : [ack]);
						}
					},
					activity
				)
			).rejects.toThrow('committed result');
		}
	);
	it('preserves transport and project-transition rejection', async () => {
		const failure = new Error('project replaced before dispatch');
		await expect(
			invokeWorkbenchAppOperation(
				request,
				{
					async sendIntent() {
						throw failure;
					}
				},
				activity
			)
		).rejects.toBe(failure);
	});
});
