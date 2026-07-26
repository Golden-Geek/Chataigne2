import { describe, expect, it, vi } from 'vitest';
import { CanvasRenderScheduler } from '../canvas-render-scheduler';

describe('CanvasRenderScheduler', () => {
	it('coalesces packed telemetry updates into one animation frame', () => {
		let frame: FrameRequestCallback | null = null;
		const request = vi.fn((callback: FrameRequestCallback) => {
			frame = callback;
			return 7;
		});
		const cancel = vi.fn();
		const scheduler = new CanvasRenderScheduler(request, cancel);
		const stale = vi.fn();
		const latest = vi.fn();

		scheduler.request(stale);
		scheduler.request(latest);
		expect(request).toHaveBeenCalledOnce();
		const capturedFrame = frame as unknown as FrameRequestCallback;
		capturedFrame(0);

		expect(stale).not.toHaveBeenCalled();
		expect(latest).toHaveBeenCalledOnce();
	});

	it('cancels the pending frame and rejects future work after teardown', () => {
		const request = vi.fn(() => 13);
		const cancel = vi.fn();
		const scheduler = new CanvasRenderScheduler(request, cancel);

		scheduler.request(() => {});
		scheduler.dispose();
		scheduler.request(() => {});

		expect(cancel).toHaveBeenCalledWith(13);
		expect(request).toHaveBeenCalledOnce();
	});
});
