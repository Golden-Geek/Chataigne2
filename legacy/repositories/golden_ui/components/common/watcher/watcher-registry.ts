import type { ParamValue } from '../../../types';
import WatcherColorVisualizer from './WatcherColorVisualizer.svelte';
import WatcherEventVisualizer from './WatcherEventVisualizer.svelte';
import WatcherValueVisualizer from './WatcherValueVisualizer.svelte';
import WatcherVec2Visualizer from './WatcherVec2Visualizer.svelte';
import WatcherVec3Visualizer from './WatcherVec3Visualizer.svelte';

export type WatcherVisualizerId = 'value' | 'events' | 'color' | 'vec2' | 'vec3';

export interface WatcherVisualizerEntry {
	id: WatcherVisualizerId;
	label: string;
	component: any;
	supportsRangeMode: boolean;
	supportsDecimation: boolean;
}

const valueVisualizer: WatcherVisualizerEntry = {
	id: 'value',
	label: 'Value Curves',
	component: WatcherValueVisualizer,
	supportsRangeMode: true,
	supportsDecimation: true
};

const eventsVisualizer: WatcherVisualizerEntry = {
	id: 'events',
	label: 'Change Timeline',
	component: WatcherEventVisualizer,
	supportsRangeMode: false,
	supportsDecimation: false
};

const colorVisualizer: WatcherVisualizerEntry = {
	id: 'color',
	label: 'Color Gradient',
	component: WatcherColorVisualizer,
	supportsRangeMode: false,
	supportsDecimation: false
};

const vec2Visualizer: WatcherVisualizerEntry = {
	id: 'vec2',
	label: 'Vec2 Phase Space',
	component: WatcherVec2Visualizer,
	supportsRangeMode: true,
	supportsDecimation: false
};

const vec3Visualizer: WatcherVisualizerEntry = {
	id: 'vec3',
	label: 'Vec3 Projections',
	component: WatcherVec3Visualizer,
	supportsRangeMode: true,
	supportsDecimation: false
};

export const watcherVisualizerRegistry: Record<ParamValue['kind'], WatcherVisualizerEntry> = {
	trigger: eventsVisualizer,
	int: valueVisualizer,
	float: valueVisualizer,
	str: eventsVisualizer,
	file: eventsVisualizer,
	enum: eventsVisualizer,
	bool: eventsVisualizer,
	css_value: valueVisualizer,
	vec2: vec2Visualizer,
	vec3: vec3Visualizer,
	color: colorVisualizer,
	reference: eventsVisualizer
};
