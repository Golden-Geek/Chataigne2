import type { ContextMenuItem } from 'golden_ui';
import type { createMatrixEditor } from './editor.svelte';
import type { useDocumentSelection } from '../selection.svelte';
import type { MatrixPresetEdit } from '../media/generated/MatrixPresetEdit';

export function presetActions(
	owner: ReturnType<typeof createMatrixEditor>,
	selection: ReturnType<typeof useDocumentSelection>,
	inspectMembers: (preset: string) => void | Promise<void>
): ContextMenuItem[] {
	const session = owner.session,
		generation = session?.snapshotGeneration,
		media = owner.view.media;
	const current = () =>
		owner.session === session &&
		session?.snapshotGeneration === generation &&
		owner.view.media === media &&
		!owner.busy;
	const run = async (edit: MatrixPresetEdit) => {
		if (!current()) return;
		const select = selection.capture();
		const result = await owner.edit({ kind: 'Preset', edit });
		if (result?.kind === 'MatrixPresetCreated') select(result.preset);
	};
	return [
		{ label: 'Capture configuration', action: () => run({ kind: 'Capture', label: null }) },
		...(owner.matrixWindow?.presets ?? []).map((preset): ContextMenuItem => ({
			label: preset.label,
			submenu: [
				{ label: 'Recall configuration', action: () => run({ kind: 'Recall', preset: preset.id }) },
				{
					label: 'Update from current configuration',
					action: () => run({ kind: 'Update', preset: preset.id })
				},
				{
					label: 'Inspect configuration members',
					closeOnSelect: false,
					action: () => {
						if (current()) return inspectMembers(preset.id);
					}
				},
				{
					label: 'Inspect / rename configuration',
					action: () => {
						if (current()) selection.select(preset.id);
					}
				},
				{ label: 'Delete configuration', action: () => run({ kind: 'Delete', preset: preset.id }) }
			]
		}))
	];
}
