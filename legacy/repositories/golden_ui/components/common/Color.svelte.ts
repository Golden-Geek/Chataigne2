export type Color = {
	r: number;
	g: number;
	b: number;
	a: number;
};

export class ColorUtil {
	static fromAny(value: any): Color {
		if (typeof value === 'string') {
			return this.fromHex(value);
		}
		if (Array.isArray(value)) {
			return this.fromArray(value);
		}
		if (
			value &&
			typeof value === 'object' &&
			'r' in value &&
			'g' in value &&
			'b' in value &&
			'a' in value
		) {
			return value as Color;
		}
		return { r: 0, g: 0, b: 0, a: 1 };
	}

	static fromArray(arr: number[]): Color {
		return { r: arr[0] ?? 0, g: arr[1] ?? 0, b: arr[2] ?? 0, a: arr[3] ?? 1 };
	}

	static fromHex(hex: string): Color {
		if (hex.startsWith('#')) {
			hex = hex.slice(1);
		}
		if (hex.length === 6) {
			const r = parseInt(hex.slice(0, 2), 16) / 255;
			const g = parseInt(hex.slice(2, 4), 16) / 255;
			const b = parseInt(hex.slice(4, 6), 16) / 255;
			return { r, g, b, a: 1 };
		} else if (hex.length === 8) {
			const r = parseInt(hex.slice(0, 2), 16) / 255;
			const g = parseInt(hex.slice(2, 4), 16) / 255;
			const b = parseInt(hex.slice(4, 6), 16) / 255;
			const a = parseInt(hex.slice(6, 8), 16) / 255;
			return { r, g, b, a };
		}

		return { r: 0, g: 0, b: 0, a: 1 };
	}

	static toHex(color: Color, includeAlpha: boolean = true): string {
		const hexR = Math.round(color.r * 255)
			.toString(16)
			.padStart(2, '0');
		const hexG = Math.round(color.g * 255)
			.toString(16)
			.padStart(2, '0');
		const hexB = Math.round(color.b * 255)
			.toString(16)
			.padStart(2, '0');
		const hexA = Math.round(color.a * 255)
			.toString(16)
			.padStart(2, '0');
		return `#${hexR}${hexG}${hexB}${includeAlpha ? hexA : ''}`;
	}

	static fromHSV(h: number, s: number, v: number, a: number = 1): Color {
		let r = 0,
			g = 0,
			b = 0;
		let i = Math.floor(h * 6);
		let f = h * 6 - i;
		let p = v * (1 - s);
		let q = v * (1 - f * s);
		let t = v * (1 - (1 - f) * s);
		switch (i % 6) {
			case 0:
				r = v;
				g = t;
				b = p;
				break;
			case 1:
				r = q;
				g = v;
				b = p;
				break;
			case 2:
				r = p;
				g = v;
				b = t;
				break;
			case 3:
				r = p;
				g = q;
				b = v;
				break;
			case 4:
				r = t;
				g = p;
				b = v;
				break;
			case 5:
				r = v;
				g = p;
				b = q;
				break;
		}
		return { r, g, b, a };
	}

	static toHSV(color: Color): { h: number; s: number; v: number; a: number } {
		let r = color.r,
			g = color.g,
			b = color.b;
		let max = Math.max(r, g, b),
			min = Math.min(r, g, b);
		let h = 0,
			s = 0,
			v = max;
		let d = max - min;
		s = max === 0 ? 0 : d / max;
		if (max === min) {
			h = 0; // achromatic
		} else {
			switch (max) {
				case r:
					h = (g - b) / d + (g < b ? 6 : 0);
					break;
				case g:
					h = (b - r) / d + 2;
					break;
				case b:
					h = (r - g) / d + 4;
					break;
			}
			h /= 6;
		}
		return { h: h, s: s, v: v, a: color.a };
	}

	static toArray(color: Color): number[] {
		return [color.r, color.g, color.b, color.a];
	}

	static toCSSRGB(color: Color): string {
		const r = Math.round(color.r * 255);
		const g = Math.round(color.g * 255);
		const b = Math.round(color.b * 255);
		return `rgb(${r}, ${g}, ${b})`;
	}

	static toCSSRGBA(color: Color): string {
		const r = Math.round(color.r * 255);
		const g = Math.round(color.g * 255);
		const b = Math.round(color.b * 255);
		return `rgba(${r}, ${g}, ${b}, ${color.a})`;
	}

	static toCSSHSL(color: Color): string {
		const hsv = this.toHSV(color);
		const h = Math.round(hsv.h * 360);
		const s = Math.round(hsv.s * 100);
		const l = Math.round((((2 - hsv.s) * hsv.v) / 2) * 100);
		return `hsl(${h}, ${s}%, ${l}%)`;
	}

	static toCSSHSLA(color: Color): string {
		const hsv = this.toHSV(color);
		const h = Math.round(hsv.h * 360);
		const s = Math.round(hsv.s * 100);
		const v = Math.round(hsv.v * 100);
		return `hsla(${h}, ${s}%, ${v}%, ${hsv.a})`;
	}

	//add operator == and != for Color
	static equals(c1: Color, c2: Color): boolean {
		return c1.r === c2.r && c1.g === c2.g && c1.b === c2.b && c1.a === c2.a;
	}

	static notEquals(c1: Color, c2: Color): boolean {
		return !this.equals(c1, c2);
	}

	static isCompatible(value: any): boolean {
		if (typeof value === 'string') {
			//check hex color
			return /^#([0-9A-Fa-f]{6}|[0-9A-Fa-f]{8})$/.test(value);
		}
		if (Array.isArray(value) && (value.length === 3 || value.length === 4)) {
			return value.every((v) => typeof v === 'number');
		}
		if (
			value &&
			typeof value === 'object' &&
			'r' in value &&
			'g' in value &&
			'b' in value &&
			'a' in value
		) {
			return (
				typeof value.r === 'number' &&
				typeof value.g === 'number' &&
				typeof value.b === 'number' &&
				typeof value.a === 'number'
			);
		}
		return false;
	}
}
