interface SurfaceMeasurement {
	readonly bounds: DOMRect;
	readonly width: number;
	readonly height: number;
	readonly fontSize: number;
	readonly overflowX: string;
	readonly overflowY: string;
}

/** One page's read snapshot for a synchronous presentation batch, never a persistent DOM cache. */
export class SurfaceMeasurements {
	#values = new WeakMap<HTMLElement, SurfaceMeasurement>();
	#queued = false;
	read(element: HTMLElement): SurfaceMeasurement {
		const cached = this.#values.get(element);
		if (cached) return cached;
		if (!this.#queued) {
			this.#queued = true;
			queueMicrotask(() => {
				this.clear();
				this.#queued = false;
			});
		}
		const style = getComputedStyle(element);
		const measurement = {
			bounds: element.getBoundingClientRect(),
			width: element.clientWidth,
			height: element.clientHeight,
			fontSize: Number.parseFloat(style.fontSize),
			overflowX: style.overflowX,
			overflowY: style.overflowY
		};
		this.#values.set(element, measurement);
		return measurement;
	}
	clear(): void {
		this.#values = new WeakMap();
	}
}
