import CheckboxEditor from './parameters/CheckboxEditor.svelte';
import ColorPickerEditor from './parameters/ColorPickerEditor.svelte';
import CssValueEditor from './parameters/CssValueEditor.svelte';
import DropdownEditor from './parameters/DropdownEditor.svelte';
import NumberEditor from './parameters/NumberEditor.svelte';
import TextInputEditor from './parameters/TextInputEditor.svelte';
import TriggerEditor from './parameters/TriggerEditor.svelte';
import ReferenceEditor from './parameters/ReferenceEditor.svelte';
import MultiNumberEditor from './parameters/MultiNumberEditor.svelte';
import FilePathEditor from './parameters/FilePathEditor.svelte';
import type { UiNodeDto } from '../../../types';

export const propertiesInspectorClass: Record<string, { component: any }> = {
	trigger: { component: TriggerEditor },
	int: { component: NumberEditor },
	str: { component: TextInputEditor },
	file: { component: FilePathEditor },
	float: { component: NumberEditor },
	bool: { component: CheckboxEditor },
	css_value: { component: CssValueEditor },
	color: { component: ColorPickerEditor },
	enum: { component: DropdownEditor },
	vec2: { component: MultiNumberEditor },
	vec3: { component: MultiNumberEditor },
	reference: { component: ReferenceEditor }
};

export const resolveParameterEditor = (
	node: UiNodeDto | null | undefined,
	_nodesById: Map<number, UiNodeDto> | null | undefined
): { component: any } | null => {
	if (!node || node.data.kind !== 'parameter') {
		return null;
	}

	return propertiesInspectorClass[node.data.param.value.kind] ?? null;
};
