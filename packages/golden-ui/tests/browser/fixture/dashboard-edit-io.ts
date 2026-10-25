import type { UiEditIntent, EventTime } from '../../../types';
import type { createFixtureSession } from './session';

/** Controlled receipts from the real session pipeline; no native input is synthesized. */
export function createDashboardEditIO(
	fixture: ReturnType<typeof createFixtureSession>,
	time: () => EventTime
) {
	let holdBegin = false,
		holdWrites = false,
		rejectWrites = false,
		holdEvents = false;
	const records: UiEditIntent[] = [];
	const pending: (() => void)[] = [];
	const events: (() => void)[] = [];
	return {
		configure(
			options: {
				holdBegin?: boolean;
				holdWrites?: boolean;
				rejectWrites?: boolean;
				holdEvents?: boolean;
			} = {}
		) {
			if (pending.length || events.length)
				throw new Error('Resolve edit receipts/events before resetting the fixture');
			holdBegin = options.holdBegin ?? false;
			holdWrites = options.holdWrites ?? false;
			rejectWrites = options.rejectWrites ?? false;
			holdEvents = options.holdEvents ?? false;
			records.length = 0;
		},
		handle(intent: UiEditIntent) {
			if (intent.kind !== 'beginEdit' && intent.kind !== 'endEdit' && intent.kind !== 'setParam')
				return null;
			records.push(structuredClone(intent));
			const rejected = rejectWrites && intent.kind === 'setParam';
			const eventTime = intent.kind === 'setParam' && !rejected ? time() : undefined;
			if (intent.kind === 'setParam' && !rejected) {
				const apply = () => {
					const node = fixture.session.graph.state.nodesById.get(intent.node);
					if (!node || node.data.kind !== 'parameter')
						throw new Error('Edit reached a retired parameter');
					fixture.session.graph.applyEvent({
						time: eventTime!,
						kind: {
							kind: 'paramChanged',
							param: intent.node,
							old_value: node.data.param.value,
							new_value: intent.value
						}
					});
				};
				if (holdEvents) events.push(apply);
				else apply();
			}
			const acknowledgement = () =>
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
			const reply = () => ({
				...acknowledgement(),
				...(eventTime ? { earliest_event_time: eventTime, latest_event_time: eventTime } : {})
			});
			if ((intent.kind === 'beginEdit' && holdBegin) || (intent.kind === 'setParam' && holdWrites))
				return new Promise<ReturnType<typeof reply>>((resolve) => {
					pending.push(() => resolve(reply()));
				});
			return Promise.resolve(reply());
		},
		release() {
			for (const resolve of pending.splice(0)) resolve();
		},
		releaseEvents() {
			for (const apply of events.splice(0)) apply();
		},
		read() {
			return {
				records: structuredClone(records),
				pending: pending.length,
				events: events.length,
				operations: fixture.session.editOperations.size,
				active: fixture.session.hasActiveEditSession
			};
		}
	};
}
