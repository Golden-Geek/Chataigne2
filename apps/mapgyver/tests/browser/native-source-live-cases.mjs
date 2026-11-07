// Independent raw source values and CPU transfer equations; never use Golden's converter.
const encode = (value) =>
	Math.round(255 * (value <= 0.0031308 ? 12.92 * value : 1.055 * value ** (1 / 2.4) - 0.055));
const rgba = (rgb) => [...rgb.map(encode), 255];
function words(values, bytes, method) {
	const buffer = Buffer.alloc(values.length * bytes);
	values.forEach((value, i) => buffer[method](value, i * bytes));
	return [...buffer];
}
function gray(y, bits) {
	const value = (y - 16 * 2 ** (bits - 8)) / (219 * 2 ** (bits - 8));
	const linear = value < 0.081 ? value / 4.5 : ((value + 0.099) / 1.099) ** (1 / 0.45);
	return Array(3).fill(encode(linear)).concat(255);
}
export const liveCases = [
	{ label: 'initial-red', format: 87, pixel: [0, 0, 255, 255], rgba: [255, 0, 0, 255] },
	{ label: 'same-layout-green', format: 87, pixel: [0, 255, 0, 255], rgba: [0, 255, 0, 255] },
	{
		label: 'float-cyan',
		format: 2,
		pixel: words([0.125, 0.5, 0.75, 1], 4, 'writeFloatLE'),
		rgba: rgba([0.125, 0.5, 0.75])
	},
	{
		label: 'same-layout-float-pink',
		format: 2,
		pixel: words([0.75, 0.125, 0.5, 1], 4, 'writeFloatLE'),
		rgba: rgba([0.75, 0.125, 0.5])
	},
	{
		label: 'half-wide',
		format: 10,
		width: 6,
		height: 4,
		pixel: words([0x3800, 0x3a00, 0x3000, 0x3c00], 2, 'writeUInt16LE'),
		rgba: rgba([0.5, 0.75, 0.125])
	},
	{
		label: 'uint-wide',
		format: 3,
		width: 6,
		height: 4,
		pixel: words([0xc0000000, 0x80000000, 0x20000000, 0xffffffff], 4, 'writeUInt32LE'),
		rgba: rgba([0.75, 0.5, 0.125])
	},
	{
		label: 'sint-tall',
		format: 4,
		width: 4,
		height: 6,
		pixel: words([0x40000000, 0x10000000, 0x60000000, 0x7fffffff], 4, 'writeInt32LE'),
		rgba: rgba([0.5, 0.125, 0.75])
	},
	{
		label: 'nv12',
		format: 103,
		bytes: [...Array(16).fill(126), ...Array(8).fill(128)],
		rgba: gray(126, 8)
	},
	{
		label: 'p010-same-canonical-layout',
		format: 104,
		bytes: words([...Array(16).fill(300 << 6), ...Array(8).fill(512 << 6)], 2, 'writeUInt16LE'),
		rgba: gray(300, 10)
	},
	{
		label: 'nv12-wide',
		format: 103,
		width: 6,
		height: 4,
		bytes: [...Array(24).fill(180), ...Array(12).fill(128)],
		rgba: gray(180, 8)
	},
	{
		label: 'srgb-wide',
		format: 29,
		width: 8,
		height: 4,
		pixel: [128, 64, 192, 255],
		rgba: [128, 64, 192, 255]
	},
	{
		label: 'restart-float',
		format: 2,
		restart: true,
		pixel: words([0.25, 0.75, 0.125, 1], 4, 'writeFloatLE'),
		rgba: rgba([0.25, 0.75, 0.125])
	}
];
