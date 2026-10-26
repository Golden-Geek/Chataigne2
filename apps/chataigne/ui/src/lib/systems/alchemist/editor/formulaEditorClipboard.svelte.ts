import type { NodeId, UiNodeDto } from 'golden_ui';
import type { GraphNodePosition } from 'golden_graph_ui';
import { copyTextToClipboard } from 'golden_ui/utils/clipboard';
import type { FormulaFragment } from '../generated/authoring/FormulaFragment';
import { copyFormulaFragment } from '../formulaOperations';
import {
	formulaFragmentFromJson,
	formulaFragmentJson,
	findFormulaPasteOffset,
	pasteFormulaClipboard
} from '../alchemistClipboard';
import { toGraphNodes } from '../alchemistGraph';
import type { FormulaMutations, FormulaEditorSession } from './formulaMutations';

interface SaveFilePickerOptions {
	suggestedName?: string;
	types?: Array<{
		description: string;
		accept: Record<string, string[]>;
	}>;
}

interface FileSystemWritableFileStream {
	write: (data: BlobPart) => Promise<void>;
	close: () => Promise<void>;
}

interface FileSystemFileHandle {
	createWritable: () => Promise<FileSystemWritableFileStream>;
}

type SaveFilePickerWindow = Window & {
	showSaveFilePicker?: (options?: SaveFilePickerOptions) => Promise<FileSystemFileHandle>;
};

export function createFormulaEditorClipboard(
	mutations: FormulaMutations,
	source: () => {
		session: FormulaEditorSession | null;
		formula: UiNodeDto | null;
		readOnly: boolean;
		anodeNodeIds: ReadonlySet<NodeId>;
		center(): GraphNodePosition | undefined;
	},
	removeSelected: () => Promise<boolean>
) {
	let session = $derived(source().session);
	let formula = $derived(source().formula);
	let formulaReadOnly = $derived(source().readOnly);
	let graphState = $derived(session?.graph.state ?? null);
	let anodeNodeIds = $derived(source().anodeNodeIds);
	let editorDisposed = false,
		alchemistDuplicateInProgress = false;
	let alchemistClipboard: FormulaFragment | null = null;
	const clipboardTarget = () =>
		session && formula
			? {
					activeSession: session,
					owner: { node: formula.node_id, uuid: formula.uuid },
					revision: session.graphTransitionRevision,
					selectionRevision: session.selectionRevision
				}
			: null;
	type ClipboardTarget = NonNullable<ReturnType<typeof clipboardTarget>>;

	const isCurrentClipboardTarget = (target: ClipboardTarget): boolean =>
		!editorDisposed &&
		session === target.activeSession &&
		formula?.node_id === target.owner.node &&
		formula.uuid === target.owner.uuid &&
		target.activeSession.graphTransitionRevision === target.revision &&
		target.activeSession.selectionRevision === target.selectionRevision;

	const selectedClipboardNodeIds = (): NodeId[] =>
		session?.selectedNodesIds.filter((node) => anodeNodeIds.has(node)) ?? [];

	const selectedAlchemistClipboard = async (): Promise<FormulaFragment | null> => {
		const target = clipboardTarget();
		const nodes = selectedClipboardNodeIds();
		if (!target || nodes.length === 0) return null;
		return copyFormulaFragment(target.activeSession, { formula: target.owner, nodes });
	};

	const readAlchemistClipboardFromSystem = async (): Promise<FormulaFragment | null> => {
		if (typeof navigator === 'undefined' || !navigator.clipboard?.readText) return null;
		try {
			const fragment = formulaFragmentFromJson(await navigator.clipboard.readText());
			if (fragment) alchemistClipboard = fragment;
			return fragment;
		} catch (error) {
			console.warn('failed to read Alchemist clipboard text', error);
			return null;
		}
	};

	const downloadTextFile = (fileName: string, text: string): void => {
		const blob = new Blob([text], { type: 'application/json;charset=utf-8' });
		const url = URL.createObjectURL(blob);
		const link = document.createElement('a');
		link.href = url;
		link.download = fileName;
		link.rel = 'noopener';
		document.body.appendChild(link);
		link.click();
		link.remove();
		URL.revokeObjectURL(url);
	};

	const saveTextFile = async (fileName: string, text: string): Promise<void> => {
		const picker = (window as SaveFilePickerWindow).showSaveFilePicker;
		if (!picker) {
			downloadTextFile(fileName, text);
			return;
		}
		const handle = await picker({
			suggestedName: fileName,
			types: [
				{
					description: 'JSON',
					accept: { 'application/json': ['.json'] }
				}
			]
		});
		const writable = await handle.createWritable();
		await writable.write(text);
		await writable.close();
	};

	const exportSelectedAlchemistClipboard = async (): Promise<boolean> => {
		try {
			const fragment = await selectedAlchemistClipboard();
			if (!fragment) return false;
			await saveTextFile('alchemist-nodes.json', formulaFragmentJson(fragment));
			return true;
		} catch (error) {
			if (error instanceof DOMException && error.name === 'AbortError') return false;
			console.error('failed to export Alchemist clipboard JSON', error);
			return false;
		}
	};

	const duplicateClipboardIntoFormula = async (
		fragment: FormulaFragment,
		target: ClipboardTarget
	): Promise<boolean> => {
		if (!isCurrentClipboardTarget(target) || formulaReadOnly || !formula || !graphState)
			return false;
		const offset = findFormulaPasteOffset({
			fragment,
			occupied: toGraphNodes(formula, graphState.nodesById),
			viewportCenter: source().center() ?? null,
			destinationUuid: target.owner.uuid
		});
		await pasteFormulaClipboard(
			target.activeSession,
			{ formula: target.owner, fragment, offset },
			() => isCurrentClipboardTarget(target)
		);
		return true;
	};

	const copySelectedGraphItems = async (): Promise<boolean> => {
		const fragment = await selectedAlchemistClipboard();
		if (!fragment) return false;
		alchemistClipboard = fragment;
		await copyTextToClipboard(formulaFragmentJson(fragment));
		return true;
	};

	const duplicateSelectedGraphItems = async (): Promise<boolean> => {
		if (formulaReadOnly) return false;
		if (alchemistDuplicateInProgress) return true;
		const target = clipboardTarget();
		const nodes = selectedClipboardNodeIds();
		if (!target || nodes.length === 0) return false;
		let duplicated = false;
		alchemistDuplicateInProgress = true;
		try {
			await mutations.run(async () => {
				if (!isCurrentClipboardTarget(target)) return;
				const fragment = await copyFormulaFragment(target.activeSession, {
					formula: target.owner,
					nodes
				});
				duplicated = await duplicateClipboardIntoFormula(fragment, target);
			});
		} finally {
			alchemistDuplicateInProgress = false;
		}
		return duplicated;
	};

	const pasteGraphItems = async (): Promise<boolean> => {
		if (formulaReadOnly) return false;
		if (alchemistDuplicateInProgress) return true;
		const target = clipboardTarget();
		if (!target) return false;
		let pasted = false;
		alchemistDuplicateInProgress = true;
		try {
			const fragment = (await readAlchemistClipboardFromSystem()) ?? alchemistClipboard;
			if (!fragment) return false;
			await mutations.run(async () => {
				pasted = await duplicateClipboardIntoFormula(fragment, target);
			});
		} finally {
			alchemistDuplicateInProgress = false;
		}
		return pasted;
	};

	const cutSelectedGraphItems = async (): Promise<boolean> => {
		if (formulaReadOnly) return false;
		const target = clipboardTarget();
		const selected = [...(session?.selectedNodesIds ?? [])];
		if (!target || !(await copySelectedGraphItems())) return false;
		// Copy is asynchronous. A later selection or project/view change must not cut new targets.
		if (
			!isCurrentClipboardTarget(target) ||
			selected.length !== session?.selectedNodesIds.length ||
			selected.some((node, index) => node !== session?.selectedNodesIds[index])
		)
			return true;
		return removeSelected();
	};

	return {
		selectedClipboardNodeIds,
		copySelectedGraphItems,
		duplicateSelectedGraphItems,
		pasteGraphItems,
		cutSelectedGraphItems,
		exportSelectedAlchemistClipboard,
		dispose() {
			editorDisposed = true;
		}
	};
}
