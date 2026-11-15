import type { UiNodeDto, NodeId } from '../../../types';
import type { WorkbenchSession } from '../../../store/workbench.svelte';
import type { CurveNodes } from './model';
import type { createCurveOperations } from './operations';
import type { createCurveParameterWriter } from './parameter-writer';
import type { createCurveCreation } from './creation.svelte';
import type { createCurveEditGesture } from './edit-gesture.svelte';
import type { createCurveInput } from './input';
export interface CurveEditorDocument {
	readonly session: WorkbenchSession | null;
	readonly graphNodesById: CurveNodes | null;
	readonly liveNode: UiNodeDto;
	readonly input_owner_uuid: string | null;
	readonly curveNode: UiNodeDto;
}
export interface CurveEditorBindings {
	selected_key_id: NodeId | null;
	selected_key_ids: NodeId[];
	selected_curve_owner_key_ids: NodeId[];
	readonly input_selection_identity: string;
	readonly constrainKeysToNeighbors: boolean;
	readonly onselectionclear: (() => void) | undefined;
}
export interface CurveEditorServices {
	readonly curveOperations: ReturnType<typeof createCurveOperations>;
	readonly parameterWriter: ReturnType<typeof createCurveParameterWriter>;
	readonly creation: ReturnType<typeof createCurveCreation>;
	readonly editGesture: ReturnType<typeof createCurveEditGesture>;
	readonly input: ReturnType<typeof createCurveInput>;
}
