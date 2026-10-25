import assert from 'node:assert/strict';

export async function captureHeap(page, collect) {
	const connection = await page.context().newCDPSession(page);
	const read = async () => ({
		heap: await connection.send('Runtime.getHeapUsage'),
		dom: await connection.send('Memory.getDOMCounters'),
		view: await page.evaluate(() => ({
			widgets: document.querySelectorAll('.dashboard-widget-shell').length,
			connectedElements: document.querySelectorAll('*').length
		}))
	});
	try {
		const before = await read();
		if (!collect) return { before };
		// This branch is diagnostic-only and cannot run in a qualification matrix.
		await connection.send('HeapProfiler.collectGarbage');
		return { before, after: await read() };
	} finally {
		await connection.detach();
	}
}

export function verifyRetention(observations) {
	assert.deepEqual(
		observations.map((row) => row.stage),
		[
			'initial',
			'viewport',
			'dense',
			'hierarchy',
			'lifecycle',
			'edit',
			'creation',
			'gestures',
			'confirmed',
			'ordered',
			'ordered-move',
			'composition',
			'barriers'
		]
	);
	const limits = {
		heapBytes: 64 * 1024 * 1024,
		listeners: observations[0].after.dom.jsEventListeners + 32
	};
	for (const row of observations) {
		assert.equal(row.setup.authoredNodes, 1001);
		assert.equal(row.setup.widgets, 90);
		assert(
			row.after.heap.usedSize <= limits.heapBytes,
			`${row.stage}: retained heap ${row.after.heap.usedSize}`
		);
		assert(
			row.after.dom.jsEventListeners <= limits.listeners,
			`${row.stage}: retained listeners ${row.after.dom.jsEventListeners}`
		);
	}
	return limits;
}

export async function timingChecks(page, result, probe, filter) {
	assert(
		probe ||
			(!filter.cpu &&
				!filter.heap &&
				filter.tier === undefined &&
				!filter.profile &&
				!filter.action),
		'Filters/profiling are only valid in diagnostic probes'
	);
	assert(filter.tier === undefined || [1000, 10000, 20000, 50000, 100000].includes(filter.tier));
	if (filter.heap)
		assert(filter.tier && filter.profile && filter.action, 'Select one heap diagnostic workload');
	for (const tier of probe ? [filter.tier ?? 50000] : [1000, 10000, 20000, 50000, 100000])
		for (const profile of [
			'sparse',
			'dense-edit',
			'dense-run',
			'minimum-edit',
			'minimum-run',
			'mixed',
			'deep'
		]) {
			if (probe && filter.profile && filter.profile !== profile) continue;
			const beforeSetup = filter.heap ? await captureHeap(page, false) : undefined;
			const setup = await page.evaluate(
				({ tier, profile }) => window.dashboardFixture.setupTiming(tier, profile),
				{ tier, profile }
			);
			result.timingSetups.push(setup);
			console.log(JSON.stringify({ kind: 'timing-setup', ...setup }));
			if (filter.heap) {
				const observation = {
					tier,
					profile,
					beforeSetup,
					...(await captureHeap(page, true)),
					diagnosticOnly: true
				};
				(result.heapDiagnostics ??= []).push(observation);
				console.log(JSON.stringify({ kind: 'heap-diagnostic', ...observation }));
			}
			for (const action of setup.actions) {
				if (probe && filter.action && filter.action !== action) continue;
				const profiler = filter.cpu ? await page.context().newCDPSession(page) : null;
				if (profiler) {
					await profiler.send('Profiler.enable');
					await profiler.send('Profiler.start');
				}
				let value;
				try {
					value = await page.evaluate(
						(action) => window.dashboardFixture.measureTiming(action),
						action
					);
				} finally {
					if (profiler) {
						const capture = await profiler.send('Profiler.stop');
						(result.cpuProfiles ??= []).push({ tier, profile, action, capture });
						await profiler.detach();
					}
				}
				result.timings.push(value);
				console.log(
					JSON.stringify({
						kind: 'timing',
						tier,
						profile,
						action,
						summary: value.summary,
						passed: value.passed,
						failure: value.failure
							? {
									index: value.failure.index,
									message: value.failure.message,
									diagnostics: value.failure.diagnostics
								}
							: null
					})
				);
			}
		}
	if (!filter.action && !filter.profile) assert.equal(result.timings.length, probe ? 32 : 160);
	assert(result.timings.length > 0, 'No timing workloads matched');
	assert(
		result.timings.every((row) => row.passed),
		'Dashboard action matrix has failed gates; retain every measurement'
	);
}
