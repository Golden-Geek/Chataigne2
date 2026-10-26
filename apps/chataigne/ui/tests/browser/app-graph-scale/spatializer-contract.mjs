/** Fixed before action timing: changes require a documented rationale and retained failures. */
export const spatializerScaleContract = {
	version: 1,
	modes: ['voronoi', 'targetRadius', 'sourceRadius', 'overlap'],
	tiers: [1000, 10000, 20000, 50000, 100000],
	sources: 32,
	primaryActions: ['discover', 'single-move', 'three-move', 'radius', 'create', 'related-values'],
	radiusModeActions: ['single-move', 'radius'],
	actionTierCases: 60,
	warmups: 3,
	retained: 31,
	normal: { feedbackP95: 200, completionP95: 2000, maximumStall: 200 },
	stress: { feedbackP95: 400, completionP95: 4000, maximumStall: 400 },
	limits: {
		endpointGlyphs: 128,
		overviewGroups: 256,
		regionPolygons: 256,
		finderRows: 32,
		relatedRows: 42
	},
	setupDeadlineMs: 300000,
	debugPressureTiers: [50000, 100000],
	endpointCapScenes: [
		{ sources: 32, targets: 4096 },
		{ sources: 4096, targets: 32 }
	]
};

export const spatializerCalibrationScenes = [
	{ sources: 32, targets: 8 },
	{ sources: 32, targets: 16 },
	{ sources: 32, targets: 64 },
	{ sources: 8, targets: 32 },
	{ sources: 16, targets: 32 }
];
