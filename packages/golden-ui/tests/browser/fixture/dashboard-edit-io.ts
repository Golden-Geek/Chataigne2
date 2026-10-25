import type { UiEditIntent, EventTime } from '../../../types';
import type { createFixtureSession } from './session';

/** Controlled receipts from the real session pipeline; no native input is synthesized. */
export function createDashboardEditIO(
	fixture: ReturnType<typeof createFixtureSession>,
	time: () => EventTime
) {
	let holdBegin = false,
		holdWrites = false,
		rejectWrites = false;
	const records: UiEditIntent[] = [];
	const pending: (() => void)[] = [];
	return {
		configure(options: { holdBegin?: boolean; holdWrites?: boolean; rejectWrites?: boolean } = {}) {
			if (pending.length) throw new Error('Resolve edit receipts before resetting the fixture');
			holdBegin = options.holdBegin ?? false;
			holdWrites = options.holdWrites ?? false;
			rejectWrites = options.rejectWrites ?? false;
			records.length = 0;
		},
		handle(intent: UiEditIntent) {
			if (intent.kind !== 'beginEdit' && intent.kind !== 'endEdit' && intent.kind !== 'setParam')
				return null;
			records.push(structuredClone(intent));
			const rejected = rejectWrites && intent.kind === 'setParam';
			if (intent.kind === 'setParam' && !rejected) {
				const node = fixture.session.graph.state.nodesById.get(intent.node);
				if (!node || node.data.kind !== 'parameter')
					throw new Error('Edit reached a retired parameter');
				fixture.session.graph.applyEvent({
					time: time(),
					kind: {
						kind: 'paramChanged',
						param: intent.node,
						old_value: node.data.param.value,
						new_value: intent.value
					}
				});
			}
			const reply = () =>
				rejected
					? {
							...fixture.editAcknowledgement(),
							success: false,
							status: 'rejected' as const,
							error_message: 'Fixture parameter rejection'
						}
					: fixture.editAcknowledgement(
							intent.kind === 'beginEdit' ? true : intent.kind === 'endEdit' ? false : undefined
						);
			if ((intent.kind === 'beginEdit' && holdBegin) || (intent.kind === 'setParam' && holdWrites))
				return new Promise<ReturnType<typeof reply>>((resolve) => {
					pending.push(() => resolve(reply()));
				});
			return Promise.resolve(reply());
		},
		release() {
			for (const resolve of pending.splice(0)) resolve();
		},
		read() {
			return {
				records: structuredClone(records),
				pending: pending.length,
				active: fixture.session.hasActiveEditSession
			};
		}
	};
}
