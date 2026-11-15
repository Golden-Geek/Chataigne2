import type { UiNodeDto, UiParamDto } from '../types';

/** The ordinary Inspector and specialized editors share value access without changing control modes. */
export function parameterValueReadOnly(param: UiParamDto | null | undefined): boolean {
	if (!param || param.read_only) return true;
	const mode = param.control?.mode ?? 'manual';
	return mode !== 'manual' && mode !== 'templateText';
}

export function canEditParameterValue(node: UiNodeDto | null | undefined): boolean {
	return (
		!!node &&
		node.data.kind === 'parameter' &&
		node.meta.enabled &&
		!parameterValueReadOnly(node.data.param)
	);
}
