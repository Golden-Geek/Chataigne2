export type PerformanceSampleLevel = 'info' | 'warning';

export interface PerformanceSample {
	id: number;
	timestamp: number;
	level: PerformanceSampleLevel;
	category: string;
	message: string;
	data?: Record<string, unknown>;
}

const MAX_PERFORMANCE_SAMPLES = 400;

let nextPerformanceSampleId = 1;

export const performanceProfilerState = $state({
	enabled: false,
	samples: [] as PerformanceSample[]
});

export const setPerformanceProfilerEnabled = (enabled: boolean): void => {
	performanceProfilerState.enabled = enabled;
};

export const clearPerformanceSamples = (): void => {
	performanceProfilerState.samples = [];
};

export const recordPerformanceSample = (
	level: PerformanceSampleLevel,
	category: string,
	message: string,
	data?: Record<string, unknown>
): void => {
	if (!performanceProfilerState.enabled) {
		return;
	}
	performanceProfilerState.samples = [
		{
			id: nextPerformanceSampleId++,
			timestamp: Date.now(),
			level,
			category,
			message,
			data
		},
		...performanceProfilerState.samples
	].slice(0, MAX_PERFORMANCE_SAMPLES);
};
