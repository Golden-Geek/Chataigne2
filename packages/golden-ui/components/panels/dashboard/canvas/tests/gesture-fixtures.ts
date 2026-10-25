import type { FreeLayoutPreviewSet, FreeLayoutTarget } from '../contracts';

export const deferred = <T>() => {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((complete) => {
		resolve = complete;
	});
	return { promise, resolve };
};
export function sample(x = 0, width = 12, id = 10): FreeLayoutPreviewSet {
	const startPreview = {
		position: { x: 0, y: 0 },
		size: { width: { value: 12, unit: 'rem' as const }, height: { value: 4, unit: 'rem' as const } }
	};
	const target: FreeLayoutTarget = {
		nodeId: id,
		anchor: 'top-left',
		positionParamNodeId: id + 1,
		positionParamBehaviour: 'Coalesce',
		widthParamNodeId: id + 2,
		widthParamBehaviour: 'Coalesce',
		heightParamNodeId: id + 3,
		heightParamBehaviour: 'Coalesce',
		startPreview,
		startRect: { left: 0, top: 0, width: 192, height: 64 }
	};
	return {
		primaryNodeId: id,
		previews: [
			{
				target,
				preview: {
					position: { x, y: 0 },
					size: { width: { value: width, unit: 'rem' }, height: { value: 4, unit: 'rem' } }
				}
			}
		]
	};
}
export async function microtasks() {
	for (let i = 0; i < 6; i++) await Promise.resolve();
}
