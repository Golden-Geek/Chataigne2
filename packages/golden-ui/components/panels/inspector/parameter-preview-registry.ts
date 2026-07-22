import type { UiNodeDto } from '../../../types';

export interface ParameterContextPreview {
	text: string;
	label?: string;
	placement: 'value' | 'below';
}

export type ParameterContextPreviewResolver = (node: UiNodeDto) => ParameterContextPreview | null;

const resolvers: ParameterContextPreviewResolver[] = [];

export const registerParameterContextPreviewResolver = (
	resolver: ParameterContextPreviewResolver
): (() => void) => {
	resolvers.push(resolver);
	return () => {
		const index = resolvers.indexOf(resolver);
		if (index >= 0) resolvers.splice(index, 1);
	};
};

export const resolveParameterContextPreview = (node: UiNodeDto): ParameterContextPreview | null => {
	for (let index = resolvers.length - 1; index >= 0; index -= 1) {
		const preview = resolvers[index]?.(node);
		if (preview) return preview;
	}
	return null;
};
