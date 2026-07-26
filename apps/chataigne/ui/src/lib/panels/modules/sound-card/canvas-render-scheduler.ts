export type CanvasFrameRequest = (callback: FrameRequestCallback) => number;
export type CanvasFrameCancel = (handle: number) => void;

export class CanvasRenderScheduler {
	private frame: number | null = null;
	private nextDraw: (() => void) | null = null;
	private disposed = false;

	constructor(
		private readonly requestFrame: CanvasFrameRequest,
		private readonly cancelFrame: CanvasFrameCancel
	) {}

	request(draw: () => void): void {
		if (this.disposed) return;
		this.nextDraw = draw;
		if (this.frame !== null) return;
		this.frame = this.requestFrame(() => {
			this.frame = null;
			const next = this.nextDraw;
			this.nextDraw = null;
			next?.();
		});
	}

	dispose(): void {
		this.disposed = true;
		this.nextDraw = null;
		if (this.frame !== null) {
			this.cancelFrame(this.frame);
			this.frame = null;
		}
	}
}
