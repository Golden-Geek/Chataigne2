import { editorParameterProjection } from 'golden_ui/store/editor-parameter-projection';
import type { InstanceSnapshot } from '../media/generated/InstanceSnapshot';
import type { InstanceParameterBindings } from '../media/generated/InstanceParameterBindings';

/** Bindings come from the backend; presentation reads the same Parameters as the Inspector. */
export function projectInstanceParameters(
	instance: InstanceSnapshot,
	bindings: InstanceParameterBindings | undefined,
	parameters: ReturnType<typeof editorParameterProjection>
): InstanceSnapshot {
	if (!bindings) return instance;
	const definition = instance.definition,
		transform = definition.transform;
	return {
		...instance,
		definition: {
			...definition,
			opacity: parameters.value(bindings.opacity, 'float', definition.opacity),
			enabled: parameters.value(bindings.enabled, 'bool', definition.enabled),
			transform: {
				position: parameters.value(bindings.position, 'vec2', transform.position),
				scale: parameters.value(bindings.scale, 'vec2', transform.scale),
				rotation_degrees: parameters.value(
					bindings.rotation_degrees,
					'float',
					transform.rotation_degrees
				),
				pivot: parameters.value(bindings.pivot, 'vec2', transform.pivot)
			}
		}
	};
}
