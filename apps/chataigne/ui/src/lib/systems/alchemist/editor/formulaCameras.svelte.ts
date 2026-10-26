import type { GraphCamera } from 'golden_graph_ui';

/** Per-Formula viewport persistence; camera state never mutates the authored graph. */
export function createFormulaCameras() {
	const FORMULA_CAMERA_STORAGE_PREFIX = 'chataigne.alchemist.formula_camera:';
	let formulaCameras = $state<Record<string, GraphCamera>>({});
	const formulaCameraStorageKey = (formulaUuid: string): string =>
		`${FORMULA_CAMERA_STORAGE_PREFIX}${formulaUuid}`;

	const cameraFromStorage = (formulaUuid: string): GraphCamera | undefined => {
		if (typeof localStorage === 'undefined') return undefined;
		const raw = localStorage.getItem(formulaCameraStorageKey(formulaUuid));
		if (!raw) return undefined;
		try {
			const parsed = JSON.parse(raw) as Partial<GraphCamera>;
			const { x, y, zoom } = parsed;
			if (
				typeof x === 'number' &&
				Number.isFinite(x) &&
				typeof y === 'number' &&
				Number.isFinite(y) &&
				typeof zoom === 'number' &&
				Number.isFinite(zoom) &&
				zoom > 0
			) {
				return { x, y, zoom };
			}
		} catch {
			return undefined;
		}
		return undefined;
	};

	const persistFormulaCamera = (formulaUuid: string, camera: GraphCamera): void => {
		formulaCameras = { ...formulaCameras, [formulaUuid]: camera };
		if (typeof localStorage === 'undefined') return;
		localStorage.setItem(formulaCameraStorageKey(formulaUuid), JSON.stringify(camera));
	};

	return {
		read: (uuid: string) => formulaCameras[uuid] ?? cameraFromStorage(uuid),
		write: persistFormulaCamera
	};
}
