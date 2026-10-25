import { tick } from 'svelte';

export const nextFrame = () => new Promise<number>(requestAnimationFrame);
export function ensure(value: unknown, message: string): asserts value {
	if (!value) throw new Error(message);
}
const rounded = (value: number) => Math.round(value * 1000) / 1000;
const percentile = (values: number[], q: number) =>
	[...values].sort((a, b) => a - b)[Math.ceil(values.length * q) - 1] ?? 0;
export interface DashboardActionStep {
	feedback(): boolean;
	correct(): void;
	complete: Promise<unknown>;
	afterFeedback?(): Promise<unknown>;
}
export const presentationCounts = () => {
	const counts = {
		widgets: document.querySelectorAll('.dashboard-widget-shell').length,
		groups: document.querySelectorAll('.dashboard-free-overview rect').length,
		ordinary: 0,
		frame: 0,
		interaction: 0,
		selected: 0,
		other: 0,
		additional: 0,
		browserRows: document.querySelectorAll('.dashboard-child-menu .gc-context-item').length
	};
	for (const marker of document.querySelectorAll<HTMLElement>('[data-dashboard-budget]'))
		for (const key of ['ordinary', 'frame', 'interaction', 'selected', 'other'] as const)
			counts[key] += Number(marker.getAttribute(`data-dashboard-detail-${key}`) ?? 0);
	counts.additional = counts.frame + counts.interaction + counts.selected + counts.other;
	return counts;
};
type Counts = ReturnType<typeof presentationCounts>;
interface DashboardSample {
	feedbackMs: number;
	completeMs: number;
	intervalsMs: number[];
	peak: Counts;
	final: Counts;
}
export async function measureDashboardAction(context: {
	name: string;
	size: number;
	profile: string;
	prepare(index: number): Promise<void>;
	start(index: number): Promise<DashboardActionStep>;
	idle(): boolean;
	diagnostics(): unknown;
}) {
	const warmups: DashboardSample[] = [],
		samples: DashboardSample[] = [];
	let failure: unknown = null;
	for (let index = 0; index < 34; index++) {
		try {
			await context.prepare(index);
		} catch (error) {
			failure = {
				index,
				stage: 'preparation',
				message: String(error),
				diagnostics: context.diagnostics()
			};
			break;
		}
		let previous = await nextFrame(),
			active = true,
			handle = 0;
		const intervalsMs: number[] = [],
			peak = presentationCounts();
		const count = () => {
			const current = presentationCounts();
			for (const key of Object.keys(peak) as (keyof Counts)[])
				peak[key] = Math.max(peak[key], current[key]);
		};
		const observe = (now: number) => {
			intervalsMs.push(rounded(now - previous));
			previous = now;
			count();
			if (active) handle = requestAnimationFrame(observe);
		};
		const observer = new MutationObserver(count);
		observer.observe(document.querySelector('main')!, {
			subtree: true,
			childList: true,
			attributes: true,
			attributeFilter: [
				'class',
				'style',
				'data-dashboard-detail-ordinary',
				'data-dashboard-detail-frame',
				'data-dashboard-detail-interaction',
				'data-dashboard-detail-selected',
				'data-dashboard-detail-other'
			]
		});
		handle = requestAnimationFrame(observe);
		const started = performance.now();
		try {
			const until = async (condition: () => boolean, message: string) => {
				for (;;) {
					await tick();
					if (condition()) return;
					ensure(performance.now() - started < 5000, message);
					await nextFrame();
				}
			};
			const step = await context.start(index);
			void step.complete.catch(() => {});
			await until(step.feedback, 'No correct rendered feedback within five seconds');
			const feedbackMs = rounded(performance.now() - started);
			await step.afterFeedback?.();
			await step.complete;
			await until(context.idle, 'Dashboard operation did not settle within five seconds');
			step.correct();
			for (let frame = 0; frame < 2; frame++) {
				await nextFrame();
				await tick();
				ensure(context.idle(), 'Dashboard operation resumed during settlement');
				step.correct();
			}
			const completeMs = rounded(performance.now() - started);
			await nextFrame();
			count();
			const final = presentationCounts(),
				hierarchy = context.profile === 'mixed' || context.profile === 'deep';
			ensure(peak.browserRows <= 42, 'Child browser mount budget exceeded');
			ensure(peak.ordinary <= 512, 'Page ordinary-detail budget exceeded');
			ensure(
				peak.widgets <= (hierarchy ? 528 : context.profile === 'sparse' ? 96 : 192),
				'Widget mount budget exceeded'
			);
			ensure(
				peak.widgets + peak.groups <= (hierarchy ? 1040 : 448),
				'Combined presentation budget exceeded'
			);
			if (!hierarchy) ensure(peak.groups <= 256, 'Dense overview budget exceeded');
			if (hierarchy) ensure(peak.additional <= 16, 'Additional hierarchy cohort budget exceeded');
			(index < 3 ? warmups : samples).push({ feedbackMs, completeMs, intervalsMs, peak, final });
		} catch (error) {
			failure = {
				index,
				elapsedMs: rounded(performance.now() - started),
				message: String(error),
				intervalsMs,
				peak,
				final: presentationCounts(),
				diagnostics: context.diagnostics()
			};
			break;
		} finally {
			active = false;
			cancelAnimationFrame(handle);
			observer.disconnect();
		}
	}
	const intervals = samples.flatMap((sample) => sample.intervalsMs);
	const summary = {
		feedbackP95: percentile(
			samples.map((row) => row.feedbackMs),
			0.95
		),
		completionP95: percentile(
			samples.map((row) => row.completeMs),
			0.95
		),
		intervalP95: percentile(intervals, 0.95),
		intervalMax: intervals.reduce((max, value) => Math.max(max, value), 0)
	};
	const stress = context.size === 100000;
	const limits = {
		feedbackP95: stress ? 400 : 200,
		completionP95: stress ? 4000 : 2000,
		intervalMax: stress ? 400 : 200
	};
	return {
		action: context.name,
		size: context.size,
		profile: context.profile,
		warmups,
		samples,
		summary,
		limits,
		failure,
		passed:
			!failure &&
			samples.length === 31 &&
			summary.feedbackP95 <= limits.feedbackP95 &&
			summary.completionP95 <= limits.completionP95 &&
			summary.intervalMax <= limits.intervalMax
	};
}
