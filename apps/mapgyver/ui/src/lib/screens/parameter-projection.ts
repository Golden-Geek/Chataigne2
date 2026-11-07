import type { editorParameterProjection } from 'golden_ui/store/editor-parameter-projection';
import type { SurfaceSnapshot } from './generated/SurfaceSnapshot';
import type { SurfaceParameterBindings } from './generated/SurfaceParameterBindings';

/** Bindings expose authored presentation only; mapping validity and source policy stay backend-owned. */
export function projectSurface(
	value: SurfaceSnapshot,
	binding: SurfaceParameterBindings | undefined,
	parameters: ReturnType<typeof editorParameterProjection>
): SurfaceSnapshot {
	if (!binding) return value;
	const definition = value.definition;
	const origin = parameters.value(binding.crop_origin, 'vec2', [
		definition.crop.x,
		definition.crop.y
	]);
	const size = parameters.value(binding.crop_size, 'vec2', [
		definition.crop.width,
		definition.crop.height
	]);
	const corners = definition.quad.corners.map((point, index) => {
		const projected = parameters.value(binding.corners[index], 'vec2', [point.x, point.y]);
		return { x: projected[0], y: projected[1] };
	}) as typeof definition.quad.corners;
	return {
		...value,
		definition: {
			...definition,
			crop: { x: origin[0], y: origin[1], width: size[0], height: size[1] },
			quad: { corners },
			opacity: parameters.value(binding.opacity, 'float', definition.opacity)
		}
	};
}
