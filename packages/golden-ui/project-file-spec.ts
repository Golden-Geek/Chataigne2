import type { UiProjectFileSpec } from './types';

/** Preserve authoritative metadata; absent or malformed formats cannot become UI defaults. */
export const parseProjectFileSpec = (value: unknown): UiProjectFileSpec => {
	if (typeof value !== 'object' || value === null) {
		throw new Error('Runtime project file metadata is invalid');
	}
	const spec = value as Partial<UiProjectFileSpec>;
	if (
		typeof spec.display_name !== 'string' ||
		spec.display_name.trim().length === 0 ||
		typeof spec.extension !== 'string' ||
		spec.extension.trim().length === 0 ||
		spec.extension.startsWith('.') ||
		spec.extension !== spec.extension.trim() ||
		(spec.current_path != null &&
			(typeof spec.current_path !== 'string' || spec.current_path.trim().length === 0))
	) {
		throw new Error('Runtime project file metadata is invalid');
	}
	return {
		display_name: spec.display_name,
		extension: spec.extension,
		current_path: spec.current_path ?? null
	};
};
