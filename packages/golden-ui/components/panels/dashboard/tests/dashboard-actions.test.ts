import { beforeEach, describe, expect, it, vi } from 'vitest';
import { moveDashboardWidgetToIndex, moveDashboardWidgetToSurface } from '../dashboard-actions';
import { sendMoveDashboardWidgetIntent } from '../../../../store/ui-intents';
import { graphFixture } from '../canvas/tests/fixtures';

vi.mock('../../../../store/ui-intents', () => ({
	sendMoveDashboardWidgetIntent: vi.fn(async () => true),
	sendBindDashboardGenericWidgetTargetIntent: vi.fn(),
	sendBindDashboardNodeWidgetTargetIntent: vi.fn(),
	sendCreateDashboardContainerWidgetIntent: vi.fn(),
	sendCreateDashboardGenericWidgetIntent: vi.fn(),
	sendCreateDashboardNodeWidgetIntent: vi.fn(),
	sendDuplicateNodeIntent: vi.fn(),
	sendWrapDashboardWidgetInContainerIntent: vi.fn()
}));

describe('dashboard layout intent boundary', () => {
	beforeEach(() => vi.clearAllMocks());
	it('sends a move and complete placement as one semantic intent', async () => {
		const graph = graphFixture(2);
		const placement = {
			anchor: 'center' as const,
			position: { x: 25, y: 50 },
			size: {
				width: { value: 40, unit: 'percent' as const },
				height: { value: 7, unit: 'em' as const }
			},
			sizeEnabled: { width: true, height: false }
		};
		expect(await moveDashboardWidgetToSurface(() => graph, 10, 1, placement, 0)).toBe(true);
		expect(sendMoveDashboardWidgetIntent).toHaveBeenCalledExactlyOnceWith(
			10,
			1,
			0,
			{
				anchor: 'center',
				position: [25, 50],
				width: placement.size.width,
				height: placement.size.height,
				size_enabled: placement.sizeEnabled
			},
			undefined
		);
	});
	it('leaves target index clamping and ordering to the authoritative backend', async () => {
		const graph = graphFixture(2);
		expect(await moveDashboardWidgetToIndex(() => graph, 10, 999)).toBe(true);
		expect(sendMoveDashboardWidgetIntent).toHaveBeenCalledExactlyOnceWith(10, 1, 999);
	});
	it('propagates rejection and refuses a locally retired source', async () => {
		const graph = graphFixture(1);
		vi.mocked(sendMoveDashboardWidgetIntent).mockResolvedValueOnce(false);
		expect(await moveDashboardWidgetToSurface(() => graph, 10, 1)).toBe(false);
		expect(await moveDashboardWidgetToSurface(() => graph, 99, 1)).toBe(false);
		expect(sendMoveDashboardWidgetIntent).toHaveBeenCalledTimes(1);
	});
});
