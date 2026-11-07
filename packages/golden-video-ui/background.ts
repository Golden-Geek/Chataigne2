import type { PreviewRequest } from './generated/PreviewRequest';
type CanvasColor = Extract<PreviewRequest, { kind: 'Layout' }>['background'];

/** One cached CSS colour sample per window session; never a video frame readback. */
export function createPreviewBackground(): () => CanvasColor {
	let previous = '';
	let color: CanvasColor = [0, 0, 0, 255];
	let context: CanvasRenderingContext2D | null = null;
	return () => {
		if (typeof document === 'undefined') return color;
		const css =
			getComputedStyle(document.documentElement).getPropertyValue('--gc-color-panel').trim() ||
			'#252525';
		if (css === previous) return color;
		if (!context) {
			const sample = document.createElement('canvas');
			sample.width = sample.height = 1;
			context = sample.getContext('2d', { willReadFrequently: true });
		}
		if (!context) throw Error('Cannot resolve native canvas colour');
		context.clearRect(0, 0, 1, 1);
		context.fillStyle = '#252525';
		context.fillStyle = css;
		context.fillRect(0, 0, 1, 1);
		const rgba = context.getImageData(0, 0, 1, 1).data;
		color = [rgba[0], rgba[1], rgba[2], rgba[3]];
		previous = css;
		return color;
	};
}
