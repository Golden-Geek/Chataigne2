import { describe, expect, it } from 'vitest';
import {
	registerReferenceChoiceProvider,
	resolveReferenceChoiceProvider,
	type ReferenceChoiceProvider
} from '../reference-choice-provider';
import type { UiNodeDto } from '../../../../types';
import type { WorkbenchSession } from '../../../../store/workbench.svelte';

describe('public bounded reference choices', () => {
	it('scopes providers to reference Parameters and lets a replacement outlive old disposal', () => {
		const node = {
			data: { kind: 'parameter', param: { value: { kind: 'reference' } } }
		} as UiNodeDto;
		const session = {} as WorkbenchSession;
		const provider = (id: string): ReferenceChoiceProvider => ({
			id,
			matches: () => true,
			read: async () => {
				throw new Error('not requested');
			},
			select: async () => {}
		});
		const first = provider('test.reference.choices');
		const disposeFirst = registerReferenceChoiceProvider(first);
		expect(resolveReferenceChoiceProvider(node, session)).toBe(first);
		expect(resolveReferenceChoiceProvider(node, null)).toBeNull();
		expect(
			resolveReferenceChoiceProvider({ data: { kind: 'node' } } as unknown as UiNodeDto, session)
		).toBeNull();
		const replacement = provider(first.id);
		const disposeReplacement = registerReferenceChoiceProvider(replacement);
		disposeFirst();
		expect(resolveReferenceChoiceProvider(node, session)).toBe(replacement);
		disposeReplacement();
		expect(resolveReferenceChoiceProvider(node, session)).toBeNull();
	});
});
