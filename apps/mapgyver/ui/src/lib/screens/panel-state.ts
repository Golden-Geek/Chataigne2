import type { ScreenEditorTarget } from './generated/ScreenEditorTarget';
import type { ScreenEditorKind } from './generated/ScreenEditorKind';
export interface ScreenPanelState {
	document: string | null;
	target: ScreenEditorTarget | null;
	locked: boolean;
	surface: string | null;
}
const uuid = (value: unknown): value is string =>
	typeof value === 'string' && /^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(value);
export function screenPanelState(
	value: Partial<ScreenPanelState> = {},
	editor?: ScreenEditorKind
): ScreenPanelState {
	const target = value.target;
	const valid =
		(!editor || target?.kind === editor) &&
		uuid(value.document) &&
		(target?.kind === 'Screen'
			? uuid(target.screen)
			: target?.kind === 'Surface' &&
				uuid(target.reference?.screen) &&
				uuid(target.reference?.surface));
	return {
		document: valid ? value.document! : null,
		target:
			valid && target
				? target.kind === 'Screen'
					? { kind: 'Screen', screen: target.screen }
					: { kind: 'Surface', reference: { ...target.reference } }
				: null,
		locked: !!valid && value.locked === true,
		surface: uuid(value.surface) ? value.surface : null
	};
}
