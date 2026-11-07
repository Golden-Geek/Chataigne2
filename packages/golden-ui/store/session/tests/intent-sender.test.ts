import { describe, expect, it, vi } from 'vitest';
import type { UiAck, UiEditIntent } from '../../../types';
import { createIntentSender } from '../intent-sender';
import type { WorkbenchIntentPipeline } from '../intent-pipeline';
import type { WorkbenchUserActionCoordinator } from '../action-activity.svelte';

const silent: UiEditIntent[] = [
	{ kind: 'beginEdit', client_edit_id: 'gesture', label: 'Dragging' },
	{ kind: 'endEdit', client_edit_id: 'gesture' },
	{ kind: 'setParam', node: 1, value: { kind: 'float', value: 0.5 }, behaviour: 'Coalesce' },
	{ kind: 'setParamComponent', node: 2, component: 1, value: 0.5, behaviour: 'Coalesce' },
	{ kind: 'resizeDashboardWidget', widget: 3 },
	{ kind: 'sendNodeEvent', node: 4, topic: 'preview-demand', payload: { active: true } }
];
const ack: UiAck = { success: true, status: 'applied' };

function fixture() {
	let complete!: () => void;
	let fail!: (error: Error) => void;
	const pending = new Promise<void>((resolve, reject) => {
		complete = resolve;
		fail = reject;
	});
	const pipeline: WorkbenchIntentPipeline = {
		sendIntent: vi.fn<WorkbenchIntentPipeline['sendIntent']>(async (_intent, progress) => {
			await pending;
			progress?.applied?.([ack]);
		}),
		sendIntents: vi.fn<WorkbenchIntentPipeline['sendIntents']>(async (_intents, progress) => {
			await pending;
			progress?.applied?.([ack]);
		}),
		suspendDispatch: () => () => {},
		blockDispatch: () => () => {},
		flush: async () => {},
		runProjectGraphTransition: (operation) => operation(),
		rejectPending: () => {}
	};
	const run = vi.fn<WorkbenchUserActionCoordinator['run']>(async (_label, dispatch) =>
		dispatch({})
	);
	return { sender: createIntentSender(pipeline, { run }), pipeline, run, complete, fail };
}

describe('user activity admission at the intent sender', () => {
	it.each(silent)(
		'keeps delayed $kind silent while retaining its terminal receipt',
		async (intent) => {
			const f = fixture();
			const applied = vi.fn();
			const result = f.sender.sendIntent(intent, applied);
			expect(f.pipeline.sendIntent).toHaveBeenCalledWith(intent, expect.any(Object));
			expect(f.run).not.toHaveBeenCalled();
			expect(applied).not.toHaveBeenCalled();
			f.complete();
			await result;
			expect(applied).toHaveBeenCalledExactlyOnceWith([ack]);
			expect(f.run).not.toHaveBeenCalled();
		}
	);
	it('keeps an unlabelled parameter/event batch silent without dropping its receipt', async () => {
		const f = fixture();
		const applied = vi.fn();
		const result = f.sender.sendIntents(silent.slice(2), applied);
		expect(f.run).not.toHaveBeenCalled();
		f.complete();
		await result;
		expect(applied).toHaveBeenCalledExactlyOnceWith([ack]);
	});
	it('tracks an explicitly labelled batch once through its real terminal receipt', async () => {
		const f = fixture();
		const applied = vi.fn();
		const result = f.sender.sendIntents(silent.slice(0, 3), applied);
		expect(f.run).toHaveBeenCalledTimes(1);
		expect(f.run.mock.calls[0][0]).toBe('Dragging');
		expect(applied).not.toHaveBeenCalled();
		f.complete();
		await result;
		expect(applied).toHaveBeenCalledExactlyOnceWith([ack]);
	});
	it('tracks explicit app work even when batched with silent preview traffic', async () => {
		const f = fixture();
		const result = f.sender.sendIntents([
			silent[5],
			{ kind: 'invokeAppOperation', namespace: 'fixture', operation: 'edit', payload: {} }
		]);
		expect(f.run).toHaveBeenCalledTimes(1);
		expect(f.run.mock.calls[0][0]).toBe('Applying changes');
		f.complete();
		await result;
	});
	it('propagates a silent operation failure to its caller', async () => {
		const f = fixture();
		const failure = new Error('connection replaced');
		const result = f.sender.sendIntent(silent[2]);
		const rejected = expect(result).rejects.toBe(failure);
		f.fail(failure);
		await rejected;
		expect(f.run).not.toHaveBeenCalled();
	});
});
