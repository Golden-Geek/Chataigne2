import ts from 'typescript';
import { compileModule } from 'svelte/compiler';
// @ts-expect-error Svelte's internal client runtime is intentionally untyped.
import * as svelteClient from 'svelte/internal/client';
import { describe, expect, it } from 'vitest';
import type { UiEventDto, NodeId } from '../../../../../../packages/golden-ui/types';
import customEventStoreSource from '../../../../../../packages/golden-ui/store/session/custom-events.svelte.ts?raw';

const {
	effect,
	effect_root,
	flush: flushClient
} = svelteClient as unknown as {
	effect: (callback: () => void) => unknown;
	effect_root: (callback: () => void) => () => void;
	flush: () => void;
};

interface ClientCustomEventStore {
	getCustomEventSequence(topic: string, origin?: NodeId | null): number;
	getCustomEventPayload<T = unknown>(topic: string, origin?: NodeId | null): T | null;
	applyBatchEvents(events: UiEventDto[]): void;
	reset(): void;
}

// The default Node test transform uses Svelte's SSR output, where effects are omitted.
// Compile the production rune module for the client so this regression exercises its real signals.
const compileClientStoreFactory = (): (() => ClientCustomEventStore) => {
	const javascript = ts.transpileModule(customEventStoreSource, {
		compilerOptions: {
			module: ts.ModuleKind.ESNext,
			target: ts.ScriptTarget.ESNext
		}
	}).outputText;
	const compiled = compileModule(javascript, {
		filename: 'custom-events.svelte.js',
		generate: 'client',
		dev: false
	}).js.code;
	const executable = compiled
		.replace(/^import \* as \$ from ['"]svelte\/internal\/client['"];\s*/m, '')
		.replace(/\bexport const /g, 'const ');
	const evaluate = new Function(
		'$',
		`${executable}\nreturn { createWorkbenchCustomEventStore };`
	) as (runtime: unknown) => {
		createWorkbenchCustomEventStore: () => ClientCustomEventStore;
	};
	return evaluate(svelteClient).createWorkbenchCustomEventStore;
};

const customEvent = (seq: number, payload: unknown): UiEventDto => ({
	time: { tick: 1, micro: 0, seq },
	kind: {
		kind: 'custom',
		topic: 'preview',
		origin: 10,
		payload,
		retention: 'transient'
	}
});

describe('workbench custom event store reset reactivity', () => {
	it('invalidates an effect retaining an active entry and binds it to the replacement entry', () => {
		const store = compileClientStoreFactory()();
		store.applyBatchEvents([customEvent(1, { frame: 1 })]);
		let sequence = -1;
		let payload: { frame: number } | null = null;
		let evaluations = 0;
		const dispose = effect_root(() => {
			effect(() => {
				evaluations += 1;
				sequence = store.getCustomEventSequence('preview', 10);
				payload = store.getCustomEventPayload<{ frame: number }>('preview', 10);
			});
		});
		flushClient();

		expect(sequence).toBe(1);
		expect(payload).toEqual({ frame: 1 });
		expect(evaluations).toBe(1);

		store.reset();
		flushClient();
		expect(sequence).toBe(0);
		expect(payload).toBeNull();
		expect(evaluations).toBe(2);

		store.applyBatchEvents([customEvent(2, { frame: 2 })]);
		flushClient();
		expect(sequence).toBe(1);
		expect(payload).toEqual({ frame: 2 });
		expect(evaluations).toBe(3);

		dispose();
	});
});
