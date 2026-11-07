import { describe, expect, it } from 'vitest';
import {
	LatestEditGesture,
	createLatestEditGestureState,
	type GesturePresentation
} from '../latest-edit-gesture';

const deferred = <T>() => {
	let resolve!: (value: T) => void, reject!: (error: Error) => void;
	const promise = new Promise<T>((yes, no) => {
		resolve = yes;
		reject = no;
	});
	return { promise, resolve, reject };
};
const drain = async () => {
	for (let i = 0; i < 12; i++) await Promise.resolve();
};
function fixture() {
	const host = { status: 'connected', snapshotGeneration: 1, loading: { visible: false } };
	const state = createLatestEditGestureState();
	let active = false,
		starts = 0,
		ends = 0;
	let shown: GesturePresentation<{ id: string }, { basis: number }, { x: number }> | null = null;
	const samples: { x: number }[] = [],
		replies: ReturnType<typeof deferred<number>>[] = [],
		accepted: number[] = [];
	const closing = deferred<void>();
	const gesture = new LatestEditGesture(() => host, state, {
		group: () => ({
			get active() {
				return active;
			},
			async begin() {
				active = true;
				starts++;
			},
			async end() {
				ends++;
				await closing.promise;
				active = false;
			}
		}),
		move: async (
			_host,
			_reference: { id: string },
			_handle: { basis: number },
			point: { x: number }
		) => {
			samples.push(point);
			const reply = deferred<number>();
			replies.push(reply);
			return reply.promise;
		},
		accepted: (value: number) => accepted.push(value),
		active: () => {},
		finished: () => {},
		present: (value) => {
			shown = value;
		}
	});
	return {
		host,
		state,
		gesture,
		replies,
		samples,
		accepted,
		closing,
		get shown() {
			return shown;
		},
		get starts() {
			return starts;
		},
		get ends() {
			return ends;
		}
	};
}

describe('gesture presentation remains independent of authoritative acknowledgements', () => {
	it('presents input synchronously, coalesces writes and never rewinds on an older receipt', async () => {
		const f = fixture();
		expect(f.gesture.start({ id: 'owned' }, { basis: 10 })).toBe(true);
		f.gesture.move({ x: 1 });
		expect(f.shown?.sample.x).toBe(1);
		await drain();
		f.gesture.move({ x: 2 });
		f.gesture.move({ x: 3 });
		expect(f.samples).toEqual([{ x: 1 }]);
		expect(f.shown?.sample.x).toBe(3);
		f.replies[0].resolve(1);
		await drain();
		expect(f.samples).toEqual([{ x: 1 }, { x: 3 }]);
		expect(f.shown?.sample.x).toBe(3);
		f.gesture.finish();
		f.gesture.move({ x: 4 });
		expect(f.shown?.sample.x).toBe(3);
		f.replies[1].resolve(3);
		await drain();
		expect(f.shown?.sample.x).toBe(3);
		expect(f.ends).toBe(1);
		f.closing.resolve();
		await drain();
		expect(f.shown).toBeNull();
		expect(f.accepted).toEqual([1, 3]);
		expect(f.state.active).toBe(false);
		expect(f.starts).toBe(1);
	});
	it('cancel/project replacement clears transient presentation and rejects late ownership', async () => {
		const f = fixture();
		f.gesture.start({ id: 'owned' }, { basis: 10 });
		f.gesture.move({ x: 1 });
		await drain();
		f.gesture.move({ x: 2 });
		f.host.snapshotGeneration++;
		f.gesture.observe();
		expect(f.shown).toBeNull();
		f.replies[0].resolve(1);
		f.closing.resolve();
		await drain();
		expect(f.accepted).toEqual([]);
		expect(f.samples).toEqual([{ x: 1 }]);
		expect(f.shown).toBeNull();
	});
	it('a rejected final edit retires its hint without erasing the failure or inventing a result', async () => {
		const f = fixture();
		f.gesture.start({ id: 'owned' }, { basis: 10 });
		f.gesture.move({ x: 1 });
		await drain();
		f.gesture.finish();
		f.replies[0].reject(new Error('Backend rejected geometry'));
		f.closing.resolve();
		await drain();
		expect(f.shown).toBeNull();
		expect(f.accepted).toEqual([]);
		expect(f.state.error).toContain('Backend rejected geometry');
	});
});
