import type { UiColorDto, UiNodeDto } from '../../types';
import type { WorkbenchSession } from '../../store/workbench.svelte';
import { appState } from '../../store/workbench.svelte';
import type { ContextMenuItem } from './context-menu';
import colorIcon from '../../style/icons/parameter/color.svg';

/** A color editor target may be sparse; its owner decides how to author it. */
export interface NodeColorTarget {
	valid(): boolean;
	read(): UiColorDto;
	hasOverride(): boolean;
	write(color: UiColorDto | null): Promise<void>;
}

export function nodeColorTarget(session: WorkbenchSession, node: UiNodeDto): NodeColorTarget {
	const generation = session.snapshotGeneration;
	const current = () => session.graph.state.nodesById.get(node.node_id);
	return {
		valid: () =>
			appState.session === session &&
			session.status === 'connected' &&
			!session.loading.visible &&
			session.snapshotGeneration === generation &&
			current()?.uuid === node.uuid &&
			current()?.meta.user_permissions.can_edit_color === true,
		read: () =>
			current()?.meta.presentation?.color ??
			current()?.meta.presentation?.default_color ?? { r: 1, g: 1, b: 1, a: 1 },
		hasOverride: () => !!current()?.meta.presentation?.color,
		async write(color) {
			if (!this.valid()) return;
			const presentation = { ...current()?.meta.presentation };
			if (color) presentation.color = color;
			else delete presentation.color;
			await session.sendIntent({ kind: 'patchMeta', node: node.node_id, patch: { presentation } });
		}
	};
}

export function nodeColorMenuItems(
	target: NodeColorTarget,
	open: (target: NodeColorTarget, point: { x: number; y: number }) => void,
	close: () => void
): ContextMenuItem[] {
	return [
		{
			id: 'set-color',
			label: 'Set Color',
			icon: colorIcon,
			disabled: !target.valid() || appState.session?.hasActiveEditSession === true,
			action: (event) => {
				const rect = (event.currentTarget as HTMLElement | null)?.getBoundingClientRect();
				const point = { x: rect?.right ?? event.clientX, y: rect?.top ?? event.clientY };
				close();
				open(target, point);
			}
		},
		{
			id: 'reset-color',
			label: 'Reset Color',
			icon: colorIcon,
			disabled:
				!target.valid() || !target.hasOverride() || appState.session?.hasActiveEditSession === true,
			action: async () => {
				close();
				if (target.valid() && !appState.session?.hasActiveEditSession) await target.write(null);
			}
		}
	];
}
