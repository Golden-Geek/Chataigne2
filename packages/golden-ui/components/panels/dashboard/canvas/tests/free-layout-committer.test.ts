import { describe, expect, it, vi } from 'vitest';
import { createFreeLayoutCommitter } from '../free-layout-committer';
import { deferred, microtasks, sample } from './gesture-fixtures';

type Context = Parameters<typeof createFreeLayoutCommitter>[1];
function setup(overrides: Partial<Context> = {}) {
	const frames = new Set<() => void>(),
		events: string[] = [];
	let valid = true;
	const send = vi.fn(
		overrides.send ??
			(async () => {
				events.push('send');
				return true;
			})
	);
	const end = vi.fn(async () => {
		events.push('end');
	});
	const context: Context = {
		current: () => valid,
		begin: async () => {
			events.push('begin');
		},
		end,
		frame: (callback) => {
			frames.add(callback);
			return () => {
				frames.delete(callback);
			};
		},
		...overrides,
		send
	};
	const queue = createFreeLayoutCommitter(sample(), context);
	return {
		queue,
		send,
		end,
		events,
		context,
		invalidate: () => {
			valid = false;
		},
		fire: async () => {
			for (const callback of [...frames]) {
				frames.delete(callback);
				callback();
			}
			await microtasks();
		}
	};
}
describe('dashboard gesture commit ownership', () => {
	it('coalesces samples and owns the submitted values before their caller can mutate them', async () => {
		const s = setup(),
			last = sample(2, 13);
		s.queue.schedule(sample(1));
		s.queue.schedule(last);
		last.previews[0].preview.position.x = 99;
		await s.fire();
		await s.queue.finish();
		expect(s.send).toHaveBeenCalledTimes(1);
		expect(s.send.mock.calls[0][0]).toEqual([
			{ kind: 'setParam', node: 11, behaviour: 'Coalesce', value: { kind: 'vec2', value: [2, 0] } },
			{
				kind: 'setParam',
				node: 12,
				behaviour: 'Coalesce',
				value: { kind: 'css_value', unit: 'rem', value: 13 }
			}
		]);
	});
	it('waits for the admitted prefix and latest sample before closing the edit session', async () => {
		const first = deferred<boolean>(),
			send = vi.fn<Context['send']>(async () => true);
		send.mockImplementationOnce(() => first.promise);
		const s = setup({ send });
		s.queue.schedule(sample(1));
		await s.fire();
		s.queue.schedule(sample(2));
		s.queue.schedule(sample(3));
		const closed = s.queue.finish();
		expect(s.end).not.toHaveBeenCalled();
		first.resolve(true);
		await closed;
		expect(send).toHaveBeenCalledTimes(2);
		expect(send.mock.calls[1][0][0].value).toEqual({ kind: 'vec2', value: [3, 0] });
		expect(s.end).toHaveBeenCalledTimes(1);
	});
	it('drops unsent work on cancellation and rejects subsequent samples', async () => {
		const s = setup();
		s.queue.schedule(sample(1));
		await s.queue.cancel();
		s.queue.schedule(sample(2));
		await s.fire();
		expect(s.send).not.toHaveBeenCalled();
		expect(s.end).toHaveBeenCalledTimes(1);
	});
	it('does not drain a cancelled tail or run its drop action after an in-flight acknowledgement', async () => {
		const first = deferred<boolean>(),
			after = vi.fn(async () => {}),
			s = setup({ send: () => first.promise });
		s.queue.schedule(sample(1));
		await s.fire();
		s.queue.schedule(sample(2));
		const finished = s.queue.finish(after),
			cancelled = s.queue.cancel();
		expect(s.end).not.toHaveBeenCalled();
		first.resolve(true);
		await Promise.all([finished, cancelled]);
		expect(s.send).toHaveBeenCalledTimes(1);
		expect(after).not.toHaveBeenCalled();
		expect(s.end).toHaveBeenCalledTimes(1);
	});
	it('checks lifetime admission before sending and again after an acknowledgement', async () => {
		const a = setup();
		a.queue.schedule(sample(1));
		a.invalidate();
		await a.fire();
		await a.queue.finish();
		expect(a.send).not.toHaveBeenCalled();
		const first = deferred<boolean>(),
			b = setup({ send: () => first.promise });
		b.queue.schedule(sample(1));
		await b.fire();
		b.queue.schedule(sample(2));
		b.invalidate();
		first.resolve(true);
		await b.queue.finish();
		expect(b.send).toHaveBeenCalledTimes(1);
	});
	it('does not advance the delta baseline on rejected writes', async () => {
		const send = vi.fn(async () => true);
		send.mockResolvedValueOnce(false);
		const s = setup({ send });
		s.queue.schedule(sample(1));
		await s.fire();
		s.queue.schedule(sample(1));
		await s.fire();
		await s.queue.finish();
		expect(send).toHaveBeenCalledTimes(2);
	});
	it('keeps late acknowledgements isolated from a successor gesture', async () => {
		const first = deferred<boolean>(),
			a = setup({ send: () => first.promise });
		a.queue.schedule(sample(1));
		await a.fire();
		const previous = a.queue.finish();
		const b = setup({
			begin: async () => {
				await previous;
			}
		});
		b.queue.schedule(sample(1));
		await b.fire();
		expect(b.send).not.toHaveBeenCalled();
		first.resolve(true);
		await previous;
		await b.queue.finish();
		expect(b.send).toHaveBeenCalledTimes(1);
	});
	it('keeps final drop actions inside the same edit session and closes after a delayed begin', async () => {
		const ready = deferred<void>(),
			s = setup({ begin: () => ready.promise });
		const after = vi.fn(async () => {
			s.events.push('after');
		});
		s.queue.schedule(sample(1));
		const finished = s.queue.finish(after);
		expect(s.end).not.toHaveBeenCalled();
		ready.resolve();
		await finished;
		expect(s.events).toEqual(['send', 'after', 'end']);
	});
});
