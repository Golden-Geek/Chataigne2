import type { UiNodeDto } from '../../../types';
import type { WorkbenchSession } from '../../../store/workbench.svelte';

/** Presentation models, deliberately independent of an application's transport DTOs. */
export interface ReferenceChoiceRequest {
	location: string | null;
	cursor: { key: string; direction: 'previous' | 'next' } | null;
	expanded: boolean;
}
export interface ReferenceChoicePage {
	name: string;
	location: string | null;
	parent: string | null;
	previous: string | null;
	next: string | null;
	emptyName: string;
	selection: { name: string; missing: boolean } | null;
	choices: { key: string; name: string; browse: boolean }[];
}
export interface ReferenceChoiceProvider {
	id: string;
	matches(node: UiNodeDto, session: WorkbenchSession): boolean;
	read(
		session: WorkbenchSession,
		node: UiNodeDto,
		request: ReferenceChoiceRequest
	): Promise<ReferenceChoicePage | null>;
	select(session: WorkbenchSession, node: UiNodeDto, key: string | null): Promise<void>;
	revision?(session: WorkbenchSession): unknown;
}
const providers: ReferenceChoiceProvider[] = [];

/** Domain owners supply bounded reads and validated intents; the Inspector owns presentation. */
export function registerReferenceChoiceProvider(provider: ReferenceChoiceProvider) {
	const previous = providers.findIndex((entry) => entry.id === provider.id);
	if (previous >= 0) providers.splice(previous, 1);
	providers.push(provider);
	return () => {
		const index = providers.indexOf(provider);
		if (index >= 0) providers.splice(index, 1);
	};
}
export function resolveReferenceChoiceProvider(node: UiNodeDto, session: WorkbenchSession | null) {
	if (!session || node.data.kind !== 'parameter' || node.data.param.value.kind !== 'reference')
		return null;
	return providers.findLast((provider) => provider.matches(node, session)) ?? null;
}
