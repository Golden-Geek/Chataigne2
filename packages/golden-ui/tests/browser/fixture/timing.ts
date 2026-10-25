import { tick } from 'svelte';
const frame = () => new Promise<number>(requestAnimationFrame);
const rounded = (value: number) => Math.round(value * 1000) / 1000;
const percentile = (values: number[], q: number) =>
	[...values].sort((a, b) => a - b)[Math.ceil(values.length * q) - 1] ?? 0;
interface CollectionSample {
	feedbackMs: number;
	completeMs: number;
	intervalsMs: number[];
	views: { rows: number; mounted: number }[];
}

export async function measureCollectionFlow(
	name: string,
	size: number,
	operation: (index: number) => void,
	correct: (index: number) => boolean
) {
	const warmups: CollectionSample[] = [],
		samples: CollectionSample[] = [];
	for (let sample = 0; sample < 34; sample += 1) {
		let previous = await frame(),
			active = true,
			observer = 0;
		const intervalsMs: number[] = [];
		const observe = (now: number) => {
			intervalsMs.push(rounded(now - previous));
			previous = now;
			if (active) observer = requestAnimationFrame(observe);
		};
		observer = requestAnimationFrame(observe);
		const started = performance.now();
		try {
			operation(sample);
			for (;;) {
				await tick();
				if (correct(sample)) break;
				if (performance.now() - started > 5000)
					throw new Error(`${name} produced no correct visible feedback.`);
				await frame();
			}
			const feedbackMs = rounded(performance.now() - started);
			await frame();
			await tick();
			await frame();
			await tick();
			if (!correct(sample)) throw new Error(`${name} lost its expected state during settlement.`);
			const completeMs = rounded(performance.now() - started);
			await frame();
			const views = Array.from(document.querySelectorAll<HTMLElement>('.virtual-rows')).map(
				(element) => ({
					rows: Number(element.dataset.rowCount),
					mounted: Number(element.dataset.mountedRowCount)
				})
			);
			if (views.some((view) => view.mounted > 42))
				throw new Error('Collection mount bound exceeded.');
			(sample < 3 ? warmups : samples).push({ feedbackMs, completeMs, intervalsMs, views });
		} finally {
			active = false;
			cancelAnimationFrame(observer);
		}
	}
	const summary = {
		feedbackP95: percentile(
			samples.map((row) => row.feedbackMs),
			0.95
		),
		completionP95: percentile(
			samples.map((row) => row.completeMs),
			0.95
		),
		intervalP95: percentile(
			samples.flatMap((row) => row.intervalsMs),
			0.95
		),
		intervalMax: Math.max(...samples.flatMap((row) => row.intervalsMs))
	};
	const limits = {
		feedbackP95: size === 100000 ? 400 : 200,
		completionP95: size === 100000 ? 4000 : 2000,
		intervalMax: size === 100000 ? 400 : 200
	};
	return {
		name,
		size,
		warmups,
		samples,
		summary,
		limits,
		passed:
			summary.feedbackP95 <= limits.feedbackP95 &&
			summary.completionP95 <= limits.completionP95 &&
			summary.intervalMax <= limits.intervalMax
	};
}
