// Validate completed actual-app captures, without assigning performance acceptance.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { readdir, stat } from 'node:fs/promises';
import path from 'node:path';
import { createInterface } from 'node:readline';

export async function inspectNativeDiagnostics(
	pid,
	appLog,
	{ requireSoftwareUpload = true, requireVideoFrames = false, requireSourceBudget = false } = {}
) {
	const directory = process.env.MAPGYVER_NATIVE_DIAGNOSTICS;
	if (!directory) {
		assert(!requireSourceBudget, 'required source accounting needs an actual capture');
		return null;
	}
	assert(
		!/\[mapgyver-diagnostics\] capture (?:preparation failed|start failed|failed:)/.test(appLog)
	);
	const names = (await readdir(directory)).filter(
		(name) => name.startsWith(`native-${pid}-`) && name.endsWith('.jsonl')
	);
	assert(names.length > 0, 'the actual app must create an owned capture');
	const captures = [];
	const counts = {};
	let uploadBytes = 0,
		videoFrameUses = 0,
		residentVideoFrameUses = 0;
	const sourceUploads = new Map();
	const sourceClasses = Object.fromEntries(
		['decoder_pools', 'hardware_producers', 'hap_producers', 'software_producers'].map((kind) => [
			kind,
			{ allocations: 0, bytes: 0 }
		])
	);
	let sourceSnapshots = 0,
		sourcePeak = 0;
	for (const name of names.sort()) {
		const file = path.join(directory, name);
		const hash = createHash('sha256');
		const stream = createReadStream(file);
		stream.on('data', (chunk) => hash.update(chunk));
		const lines = createInterface({ input: stream, crlfDelay: Infinity });
		let header, end, lastSnapshot;
		let events = 0,
			snapshots = 0,
			previousSourcePeak = 0;
		for await (const line of lines) {
			assert(Buffer.byteLength(line) <= 65536, 'bounded metadata row');
			const row = JSON.parse(line);
			assert(!end, 'capture_end must be terminal');
			if (!header) {
				assert.equal(row.kind, 'capture');
				assert.equal(row.format_version, 1);
				assert.equal(row.process_id, pid);
				assert(row.qpc_frequency > 0);
				header = row;
			} else if (row.kind === 'diagnostic') {
				const kinds = Object.keys(row.event);
				assert.equal(kinds.length, 1);
				const kind = kinds[0];
				counts[kind] = (counts[kind] ?? 0) + 1;
				if (kind === 'SourceUse' && row.event[kind].frame) {
					videoFrameUses++;
					if (!row.event[kind].software_upload) residentVideoFrameUses++;
				}
				if (kind === 'ImageUpload') uploadBytes += row.event[kind].bytes;
				const upload = row.event[kind].software_upload;
				if (upload) {
					const identity = `${upload.stream}:${upload.sequence}`;
					if (sourceUploads.has(identity)) assert.deepEqual(sourceUploads.get(identity), upload);
					else {
						sourceUploads.set(identity, upload);
						uploadBytes += upload.bytes;
					}
				}
				events++;
			} else if (row.kind === 'snapshot') {
				assert.equal(row.process.process_id, pid);
				assert(row.process.memory.private_bytes > 0);
				assert.equal(row.process_error, null);
				if (requireSourceBudget) assert(row.source_gpu, 'source budget observation required');
				if (row.source_gpu) {
					const source = row.source_gpu;
					const amount = (value) => assert(Number.isSafeInteger(value) && value >= 0);
					for (const value of [source.limit_bytes, source.retained_bytes, source.peak_bytes])
						amount(value);
					assert(source.peak_bytes >= source.retained_bytes);
					assert(source.peak_bytes <= source.limit_bytes);
					assert(source.peak_bytes >= previousSourcePeak);
					previousSourcePeak = source.peak_bytes;
					let total = 0;
					for (const [kind, maximum] of Object.entries(sourceClasses)) {
						const current = source[kind];
						amount(current.allocations);
						amount(current.bytes);
						assert.equal(current.allocations === 0, current.bytes === 0);
						total += current.bytes;
						maximum.allocations = Math.max(maximum.allocations, current.allocations);
						maximum.bytes = Math.max(maximum.bytes, current.bytes);
					}
					assert.equal(source.retained_bytes, total);
					sourcePeak = Math.max(sourcePeak, source.peak_bytes);
					sourceSnapshots++;
				}
				lastSnapshot = row;
				snapshots++;
			} else {
				assert.equal(row.kind, 'capture_end');
				end = row;
			}
		}
		assert(header && end && lastSnapshot, 'an incomplete/partial capture cannot pass');
		assert.equal(end.summary.events, events);
		assert.equal(end.summary.snapshots, snapshots);
		assert.equal(end.summary.duration_limit, false);
		assert.equal(end.summary.diagnostics.enabled, true);
		assert.equal(end.summary.diagnostics.queued, 0);
		assert.equal(end.summary.diagnostics.dropped_full, 0);
		assert.equal(end.summary.diagnostics.clock_failures, 0);
		assert.equal(end.summary.diagnostics.written, events);
		assert.equal(end.summary.diagnostics.drained, events);
		assert.equal(lastSnapshot.native.initialized, false);
		assert.equal(lastSnapshot.allocations.plan.live, 0);
		assert.equal(lastSnapshot.allocations.owned_texel_bytes, 0);
		if (lastSnapshot.source_gpu) assert.equal(lastSnapshot.source_gpu.retained_bytes, 0);
		const bytes = (await stat(file)).size;
		assert(bytes <= header.max_bytes);
		captures.push({ file, bytes, events, snapshots, sha256: hash.digest('hex') });
	}
	for (const kind of ['FrameCpu', 'Output', 'Gpu', 'SourceUse']) {
		assert(counts[kind] > 0, `real playback must emit ${kind}`);
	}
	if (requireSoftwareUpload) {
		assert(uploadBytes > 0);
		assert(
			(counts.ImageUpload ?? 0) > 0 || sourceUploads.size > 0,
			'actual CPU upload evidence required'
		);
	}
	if (requireVideoFrames) assert(videoFrameUses > 0, 'actual timestamped Video uses required');
	if (requireSourceBudget)
		assert(sourceSnapshots > 0 && sourcePeak > 0, 'actual source allocation required');
	return {
		captures,
		counts,
		uploadBytes,
		observedSourceUploads: sourceUploads.size,
		videoFrameUses,
		residentVideoFrameUses,
		requireSoftwareUpload,
		requireVideoFrames,
		requireSourceBudget,
		sourceGpu: sourceSnapshots
			? {
					snapshots: sourceSnapshots,
					peak_admitted_bytes: sourcePeak,
					maximum_observed_classes: sourceClasses
				}
			: null,
		scope:
			'Completed actual-app event capture, ring health and native retirement; no sustained-performance, per-format allowance or source-age acceptance.'
	};
}
