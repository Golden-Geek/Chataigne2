import { describe, expect, it } from 'vitest';
import {
	childPresentationBudgets,
	dashboardPageDetailBudget,
	normalizePresentationBudget
} from '../presentation-budget';
import { node } from './fixtures';

describe('dashboard hierarchy presentation budgets', () => {
	it('normalizes invalid, fractional and excessive allowances at the view boundary', () => {
		expect([NaN, Infinity, -5, 0, 1.9, 900].map(normalizePresentationBudget)).toEqual([
			0, 0, 0, 0, 1, 512
		]);
	});
	it('allocates every ordinary shell once and divides the remainder only among containers', () => {
		const nodes = [
			node(1, 'dashboard_generic_widget'),
			node(2, 'dashboard_widget_container'),
			node(3, 'dashboard_widget_container')
		];
		expect([...childPresentationBudgets(nodes, 12).values()]).toEqual([1, 6, 5]);
		expect([...childPresentationBudgets(nodes, 3).values()]).toEqual([1, 1, 1]);
	});
	it('bounds a broad and deep free hierarchy by the original page allowance', () => {
		let sequence = 0;
		const walk = (allowance: number, depth: number): number => {
			if (depth === 0 || allowance < 2) return 0; // A dense surface shows its overview.
			const nodes = [
				node(++sequence, 'dashboard_widget_container'),
				node(++sequence, 'dashboard_widget_container')
			];
			const budgets = childPresentationBudgets(nodes, allowance);
			return nodes.reduce(
				(sum, item) => sum + 1 + walk(budgets.get(item.node_id)! - 1, depth - 1),
				0
			);
		};
		expect(walk(dashboardPageDetailBudget, 16)).toBeLessThanOrEqual(dashboardPageDetailBudget);
	});
	it('does not give retained extras new ordinary descendant slots', () => {
		const nodes = [node(1, 'dashboard_widget_container'), node(2, 'dashboard_widget_container')];
		expect([...childPresentationBudgets(nodes, 0).values()]).toEqual([1, 1]);
		expect([...childPresentationBudgets([], 512)]).toEqual([]);
	});
});
