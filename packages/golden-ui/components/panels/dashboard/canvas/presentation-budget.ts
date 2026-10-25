import type { NodeId, UiNodeDto } from '../../../../types';

export const dashboardPageDetailBudget = 512;
export const dashboardSelectedDetailLimit = 16;

export function normalizePresentationBudget(value: number): number {
	return Number.isFinite(value)
		? Math.max(0, Math.min(dashboardPageDetailBudget, Math.floor(value)))
		: 0;
}

/** Each child costs one shell; only containers receive a share of the remaining detail. */
export function childPresentationBudgets(
	children: readonly UiNodeDto[],
	available: number
): ReadonlyMap<NodeId, number> {
	const containers = children.filter((node) => node.node_type === 'dashboard_widget_container');
	const remaining = Math.max(0, normalizePresentationBudget(available) - children.length);
	const each = containers.length ? Math.floor(remaining / containers.length) : 0;
	const extra = containers.length ? remaining % containers.length : 0;
	const result = new Map(children.map((node) => [node.node_id, 1]));
	containers.forEach((node, index) => result.set(node.node_id, 1 + each + (index < extra ? 1 : 0)));
	return result;
}
