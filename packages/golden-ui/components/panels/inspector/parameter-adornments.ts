import type { Component } from 'svelte';
import type { UiNodeDto } from '../../../types';

export type ParameterAdornment = Component<{ node: UiNodeDto; readOnly: boolean }>;
type Registration = {
	id: string;
	matches: (node: UiNodeDto, nodes: Map<number, UiNodeDto>) => boolean;
	component: ParameterAdornment;
};
const registrations: Registration[] = [];

/** Add contextual actions alongside the standard typed Parameter editor. */
export function registerParameterAdornment(registration: Registration): () => void {
	const previous = registrations.findIndex((item) => item.id === registration.id);
	if (previous >= 0) registrations.splice(previous, 1);
	registrations.push(registration);
	return () => {
		const index = registrations.indexOf(registration);
		if (index >= 0) registrations.splice(index, 1);
	};
}

export function resolveParameterAdornment(
	node: UiNodeDto,
	nodes: Map<number, UiNodeDto> | null
): ParameterAdornment | null {
	if (!nodes || node.data.kind !== 'parameter') return null;
	return registrations.findLast((item) => item.matches(node, nodes))?.component ?? null;
}
