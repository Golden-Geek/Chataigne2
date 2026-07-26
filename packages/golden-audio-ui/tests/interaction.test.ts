import { describe, expect, it } from 'vitest';
import { AudioInspectorInteractionCoordinator } from '../interaction';

describe('audio inspector interaction coordination', () => {
	it('rolls a rejected intent back and reports it', async () => {
		const coordinator = new AudioInspectorInteractionCoordinator();
		let rollbackCount = 0;

		const outcome = await coordinator.submit(
			async () => false,
			() => {
				rollbackCount += 1;
			}
		);

		expect(outcome.accepted).toBe(false);
		expect(outcome.message).toContain('restored');
		expect(rollbackCount).toBe(1);
	});

	it('ignores a refresh completion after destruction', async () => {
		const coordinator = new AudioInspectorInteractionCoordinator();
		let finishRefresh: (() => void) | undefined;
		const completion = new Promise<void>((resolve) => {
			finishRefresh = resolve;
		});
		let settled = false;
		const refresh = coordinator.refresh(
			() => completion,
			() => {
				settled = true;
			}
		);

		coordinator.dispose();
		finishRefresh?.();
		await refresh;

		expect(settled).toBe(false);
	});

	it('lets only the newest refresh update presentation state', async () => {
		const coordinator = new AudioInspectorInteractionCoordinator();
		let finishFirst: (() => void) | undefined;
		const firstCompletion = new Promise<void>((resolve) => {
			finishFirst = resolve;
		});
		const settled: string[] = [];
		const first = coordinator.refresh(
			() => firstCompletion,
			() => settled.push('first')
		);
		const second = coordinator.refresh(
			async () => undefined,
			() => settled.push('second')
		);

		await second;
		finishFirst?.();
		await first;

		expect(settled).toEqual(['second']);
	});
});
