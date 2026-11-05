/** Leave the browser's rendering task before dispatching native geometry. This provides
 * a rendering opportunity, not a GPU/compositor presentation acknowledgement. */
export class RenderOpportunity {
	private pending: (() => void) | null = null;

	wait(): Promise<boolean> {
		if (typeof requestAnimationFrame !== 'function') return Promise.resolve(true);
		this.cancel();
		return new Promise((resolve) => {
			let frame: number | null = null;
			let task: ReturnType<typeof setTimeout> | null = null;
			const finish = (ready: boolean) => {
				if (this.pending !== cancel) return;
				this.pending = null;
				if (frame !== null) cancelAnimationFrame(frame);
				if (task !== null) clearTimeout(task);
				clearTimeout(deadline);
				resolve(ready);
			};
			const cancel = () => finish(false);
			this.pending = cancel;
			const deadline = setTimeout(cancel, 1500);
			frame = requestAnimationFrame(() => {
				if (this.pending !== cancel) return;
				frame = null;
				task = setTimeout(() => finish(true), 0);
			});
		});
	}

	cancel() {
		this.pending?.();
	}
}
