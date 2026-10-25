import type { UiNodeDto } from '../../../types';

/** Selection modifiers shared by pointer and keyboard callers. */
export type NodeSelectionInput = Pick<
	MouseEvent,
	'altKey' | 'ctrlKey' | 'metaKey' | 'shiftKey' | 'detail'
>;

export function nodeSelectionMode(input: NodeSelectionInput) {
	return input.ctrlKey || input.metaKey
		? 'TOGGLE'
		: input.shiftKey
			? 'ADD'
			: input.altKey
				? 'REMOVE'
				: 'REPLACE';
}

export type NodeActivation = (node: UiNodeDto, input: NodeSelectionInput) => void;
