import type { GraphNode, GraphNodePosition } from 'golden_graph_ui';
import type { WorkbenchSession } from 'golden_ui';
import type { FormulaFragment } from './generated/authoring/FormulaFragment';
import type { FormulaPasteRequest } from './generated/authoring/FormulaPasteRequest';
import { isFormulaFragment, pasteFormulaFragment } from './formulaOperations';

export const formulaFragmentJson = (fragment: FormulaFragment): string =>
	JSON.stringify(fragment, null, 2);

export const formulaFragmentFromJson = (text: string): FormulaFragment | null => {
	try {
		const value: unknown = JSON.parse(text);
		return isFormulaFragment(value) ? value : null;
	} catch {
		return null;
	}
};

type PasteSession = Pick<
	WorkbenchSession,
	'invokeAppOperation' | 'requestNodeSelection' | 'graphTransitionRevision' | 'selectionRevision'
>;

/** Selection follows the committed reply even when graph projection arrives later. */
export const pasteFormulaClipboard = async (
	session: PasteSession,
	request: FormulaPasteRequest,
	isCurrent: () => boolean = () => true
): Promise<void> => {
	const revision = session.graphTransitionRevision;
	const selectionRevision = session.selectionRevision;
	const result = await pasteFormulaFragment(session, request);
	const identities = new Map(result.identities.map((identity) => [identity.node, identity]));
	const selected = result.selected_nodes.map((node) => {
		const identity = identities.get(node);
		if (!identity) throw new Error('Formula paste selection is missing an authoritative identity.');
		return { node_id: identity.node, uuid: identity.uuid };
	});
	if (
		revision === session.graphTransitionRevision &&
		selectionRevision === session.selectionRevision &&
		isCurrent()
	) {
		session.requestNodeSelection(selected);
	}
};

interface Bounds {
	left: number;
	top: number;
	right: number;
	bottom: number;
}

// Presentation estimates only. Authored sizes and all domain defaults stay inside opaque capsules.
const NODE_WIDTH = 13;
const NODE_HEIGHT = 4.5;
const GAP = 2;
const SCAN_RINGS = 12;

const graphBounds = (node: GraphNode): Bounds => ({
	left: node.position.x,
	top: node.position.y,
	right: node.position.x + Math.max(8, node.size?.width ?? node.automaticSize?.width ?? NODE_WIDTH),
	bottom:
		node.position.y + Math.max(1, node.size?.height ?? node.automaticSize?.height ?? NODE_HEIGHT)
});

const overlaps = (a: Bounds, b: Bounds): boolean =>
	a.left < b.right + GAP &&
	a.right > b.left - GAP &&
	a.top < b.bottom + GAP &&
	a.bottom > b.top - GAP;

/** Viewport-dependent placement hint; does not inspect or modify authored documents. */
export const findFormulaPasteOffset = ({
	fragment,
	occupied,
	viewportCenter,
	destinationUuid
}: {
	fragment: FormulaFragment;
	occupied: readonly GraphNode[];
	viewportCenter: GraphNodePosition | null;
	destinationUuid: string;
}): [number, number] => {
	let bounds: Bounds | undefined;
	for (const { position } of fragment.items) {
		if (!position) continue;
		const [x, y] = position;
		bounds = bounds
			? {
					left: Math.min(bounds.left, x),
					top: Math.min(bounds.top, y),
					right: Math.max(bounds.right, x + NODE_WIDTH),
					bottom: Math.max(bounds.bottom, y + NODE_HEIGHT)
				}
			: { left: x, top: y, right: x + NODE_WIDTH, bottom: y + NODE_HEIGHT };
	}
	if (!bounds) return [0, 0];
	const rect = bounds;
	const sameFormula = fragment.source_formula_uuid === destinationUuid;
	const center = viewportCenter ?? {
		x: (rect.left + rect.right) / 2,
		y: (rect.top + rect.bottom) / 2
	};
	const base: [number, number] = sameFormula
		? [0, 0]
		: [center.x - (rect.left + rect.right) / 2, center.y - (rect.top + rect.bottom) / 2];
	const stepX = Math.max(rect.right - rect.left, NODE_WIDTH) + GAP;
	const stepY = Math.max(rect.bottom - rect.top, NODE_HEIGHT) + GAP;
	const occupiedBounds = occupied.map(graphBounds);
	const candidate = (x: number, y: number): [number, number] => [base[0] + x, base[1] + y];
	const isClear = ([x, y]: [number, number]): boolean => {
		const shifted = {
			left: rect.left + x,
			top: rect.top + y,
			right: rect.right + x,
			bottom: rect.bottom + y
		};
		return occupiedBounds.every((other) => !overlaps(shifted, other));
	};
	const preferred = sameFormula
		? [
				[stepX, 0],
				[0, stepY],
				[stepX, stepY],
				[-stepX, 0],
				[0, -stepY]
			]
		: [
				[0, 0],
				[stepX, 0],
				[0, stepY],
				[stepX, stepY]
			];
	for (const [x, y] of preferred) {
		const offset = candidate(x, y);
		if (isClear(offset)) return offset;
	}
	for (let ring = 1; ring <= SCAN_RINGS; ring++) {
		for (let y = -ring; y <= ring; y++) {
			for (let x = -ring; x <= ring; x++) {
				if (Math.max(Math.abs(x), Math.abs(y)) !== ring) continue;
				const offset = candidate(x * stepX, y * stepY);
				if (isClear(offset)) return offset;
			}
		}
	}
	return candidate(stepX, stepY);
};
