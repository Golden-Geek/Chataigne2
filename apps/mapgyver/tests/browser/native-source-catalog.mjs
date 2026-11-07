// Fixed original SDK allocation scope; independent uniform raw values/CPU SDR references.
import assert from 'node:assert/strict';

const inventory = [
	1, 2, 3, 4, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34,
	35, 36, 37, 38, 39, 41, 42, 43, 48, 49, 50, 51, 52, 53, 54, 56, 57, 58, 59, 60, 61, 62, 63, 64,
	65, 85, 86, 87, 88, 89, 90, 91, 92, 93, 100, 101, 102, 103, 104, 105, 107, 108, 109, 115
];
const clamp = (v) => Math.min(1, Math.max(0, v));
const encode = (v) => {
	v = clamp(v);
	return Math.round(255 * (v <= 0.0031308 ? 12.92 * v : 1.055 * v ** (1 / 2.4) - 0.055));
};
const base = (id) => [
	0.125 + (id % 5) / 8,
	0.25 + (Math.floor(id / 5) % 4) / 8,
	0.375 + (Math.floor(id / 20) % 4) / 8,
	1
];
const halves = new Map([
	[0.125, 0x3000],
	[0.25, 0x3400],
	[0.375, 0x3600],
	[0.5, 0x3800],
	[0.625, 0x3900],
	[0.75, 0x3a00],
	[0.875, 0x3b00],
	[1, 0x3c00]
]);
function words(values, bytes, method) {
	const result = Buffer.alloc(values.length * bytes);
	values.forEach((v, i) => result[method](v, i * bytes));
	return [...result];
}
const repeat = (pixel, count) => Array.from({ length: count }, () => pixel).flat();
const rgba = (rgb, alpha = 1) => [...rgb.map(encode), Math.round(255 * alpha)];
const rows = [];
function add(id, pixel, expected, bytes) {
	rows.push({
		label: `dxgi-${id}`,
		format: id,
		...(bytes ? { bytes } : { pixel }),
		rgba: expected
	});
}
for (const [channels, bits, entries] of [
	[
		4,
		32,
		[
			[1, 'f'],
			[2, 'f'],
			[3, 'u'],
			[4, 'i']
		]
	],
	[
		4,
		16,
		[
			[9, 'f'],
			[10, 'f'],
			[11, 'n'],
			[12, 'u'],
			[13, 's'],
			[14, 'i']
		]
	],
	[
		2,
		32,
		[
			[15, 'f'],
			[16, 'f'],
			[17, 'u'],
			[18, 'i']
		]
	],
	[
		4,
		8,
		[
			[27, 'c'],
			[28, 'c'],
			[29, 'c'],
			[30, 'u'],
			[31, 's'],
			[32, 'i']
		]
	],
	[
		2,
		16,
		[
			[33, 'f'],
			[34, 'f'],
			[35, 'n'],
			[36, 'u'],
			[37, 's'],
			[38, 'i']
		]
	],
	[
		1,
		32,
		[
			[39, 'f'],
			[41, 'f'],
			[42, 'u'],
			[43, 'i']
		]
	],
	[
		2,
		8,
		[
			[48, 'n'],
			[49, 'n'],
			[50, 'u'],
			[51, 's'],
			[52, 'i']
		]
	],
	[
		1,
		16,
		[
			[53, 'f'],
			[54, 'f'],
			[56, 'n'],
			[57, 'u'],
			[58, 's'],
			[59, 'i']
		]
	],
	[
		1,
		8,
		[
			[60, 'n'],
			[61, 'n'],
			[62, 'u'],
			[63, 's'],
			[64, 'i']
		]
	]
]) {
	for (const [id, kind] of entries) {
		const values = base(id).slice(0, channels);
		let pixel, decoded;
		if (kind === 'f') {
			decoded = values;
			pixel =
				bits === 32
					? words(values, 4, 'writeFloatLE')
					: words(
							values.map((v) => {
								assert(halves.has(v));
								return halves.get(v);
							}),
							2,
							'writeUInt16LE'
						);
		} else {
			const signed = kind === 'i' || kind === 's';
			const maximum = 2 ** (bits - Number(signed)) - 1;
			if (kind === 's' && channels > 1) values[0] *= -1;
			const codes = values.map((v) => Math.round(v * maximum));
			pixel = words(
				codes,
				bits / 8,
				`write${signed ? 'Int' : 'UInt'}${bits}LE`.replace('8LE', '8')
			);
			decoded = codes.map((v) => v / maximum);
		}
		const rgb = [...decoded.slice(0, 3)];
		while (rgb.length < 3) rgb.push(0);
		add(
			id,
			pixel,
			kind === 'c' ? [...pixel.slice(0, 3), 255] : rgba(rgb, channels === 4 ? decoded[3] : 1)
		);
	}
}
for (const id of [23, 24, 25]) {
	const values = base(id);
	const codes = values.slice(0, 3).map((v) => Math.round(v * 1023));
	add(
		id,
		words([(codes[0] | (codes[1] << 10) | (codes[2] << 20) | (3 << 30)) >>> 0], 4, 'writeUInt32LE'),
		rgba(codes.map((v) => v / 1023))
	);
}
add(
	26,
	words([(0x340 | (0x380 << 11) | (0x1d0 << 22)) >>> 0], 4, 'writeUInt32LE'),
	rgba([0.25, 0.5, 0.75])
);
// A8 stores alpha only; shader reads fill absent RGB with zero, not white.
add(65, [128], [0, 0, 0, 128]);
for (const id of [85, 86, 115]) {
	const bits = id === 85 ? [5, 6, 5] : id === 86 ? [5, 5, 5] : [4, 4, 4];
	const codes = base(id)
		.slice(0, 3)
		.map((v, i) => Math.round(v * (2 ** bits[i] - 1)));
	const alpha = id === 115 ? 9 : 1;
	const value =
		id === 85
			? (codes[0] << 11) | (codes[1] << 5) | codes[2]
			: id === 86
				? (alpha << 15) | (codes[0] << 10) | (codes[1] << 5) | codes[2]
				: (alpha << 12) | (codes[0] << 8) | (codes[1] << 4) | codes[2];
	add(
		id,
		words([value], 2, 'writeUInt16LE'),
		rgba(
			codes.map((v, i) => v / (2 ** bits[i] - 1)),
			id === 115 ? alpha / 15 : 1
		)
	);
}
for (const id of [87, 88, 90, 91, 92, 93]) {
	const rgb = base(id)
		.slice(0, 3)
		.map((v) => Math.round(v * 255));
	add(id, [rgb[2], rgb[1], rgb[0], 255], [...rgb, 255]);
}
{
	const codes = base(89)
		.slice(0, 3)
		.map((v) => 384 + Math.round(v * 510));
	add(
		89,
		words([(codes[0] | (codes[1] << 10) | (codes[2] << 20) | (3 << 30)) >>> 0], 4, 'writeUInt32LE'),
		rgba(codes.map((v) => (v - 384) / 510))
	);
}
function gray(y, bits) {
	const value = (y - 16 * 2 ** (bits - 8)) / (219 * 2 ** (bits - 8));
	const linear = value < 0.081 ? value / 4.5 : ((value + 0.099) / 1.099) ** (1 / 0.45);
	return rgba([linear, linear, linear]);
}
add(100, [128, 128, 126, 255], gray(126, 8));
add(
	101,
	words([(512 | (500 << 10) | (512 << 20) | (3 << 30)) >>> 0], 4, 'writeUInt32LE'),
	gray(500, 10)
);
add(102, words([32768, 126 * 256, 32768, 65535], 2, 'writeUInt16LE'), gray(126 * 256, 16));
add(103, null, gray(180, 8), [...Array(16).fill(180), ...Array(8).fill(128)]);
add(
	104,
	null,
	gray(300, 10),
	words([...Array(16).fill(300 << 6), ...Array(8).fill(512 << 6)], 2, 'writeUInt16LE')
);
add(
	105,
	null,
	gray(140 * 256, 16),
	words([...Array(16).fill(140 * 256), ...Array(8).fill(32768)], 2, 'writeUInt16LE')
);
add(107, null, gray(100, 8), repeat([100, 128, 100, 128], 8));
add(
	108,
	null,
	gray(600, 10),
	repeat(words([600 << 6, 512 << 6, 600 << 6, 512 << 6], 2, 'writeUInt16LE'), 8)
);
add(
	109,
	null,
	gray(180 * 256, 16),
	repeat(words([180 * 256, 32768, 180 * 256, 32768], 2, 'writeUInt16LE'), 8)
);
rows.sort((a, b) => a.format - b.format);
assert.deepEqual(
	rows.map((row) => row.format),
	inventory,
	'Fixed original70-format SDK allocation inventory'
);

export function catalogCases(adapter) {
	const nvidia = adapter.vendor_id === 0x10de && adapter.device_id === 0x2c59;
	const intel = adapter.vendor_id === 0x8086 && adapter.device_id === 0x7d67;
	assert(
		nvidia || intel,
		'This original allocation inventory covers only the recorded physical GPU pair'
	);
	const selected = rows.filter((row) => !nvidia || ![105, 115].includes(row.format));
	assert.equal(selected.length, nvidia ? 68 : 70);
	for (let i = 1; i < selected.length; ++i)
		assert.notDeepEqual(
			selected[i].rgba,
			selected[i - 1].rgba,
			'Sequential formats must reveal stale images'
		);
	return selected;
}
