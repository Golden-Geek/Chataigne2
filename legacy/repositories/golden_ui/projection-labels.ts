import type { UiParamValueProjection } from './types';

const PROJECTION_LABELS: Record<UiParamValueProjection, string> = {
	floatToVec2X0: 'Float -> X',
	floatToVec20Y: 'Float -> Y',
	floatToVec2XX: 'Float -> All',
	floatToVec3X00: 'Float -> X',
	floatToVec30Y0: 'Float -> Y',
	floatToVec300Z: 'Float -> Z',
	floatToVec3XXX: 'Float -> All',
	vec2X: 'Vec2 X',
	vec2Y: 'Vec2 Y',
	vec2ToVec3XY0: 'Vec2 -> X,Y',
	vec2ToVec3X0Y: 'Vec2 -> X,Z',
	vec2ToColorHs: 'Vec2 -> Hue,Sat',
	vec3X: 'Vec3 X',
	vec3Y: 'Vec3 Y',
	vec3Z: 'Vec3 Z',
	vec3ToVec2XY: 'Vec3 -> Vec2 (X,Y)',
	vec3ToVec2XZ: 'Vec3 -> Vec2 (X,Z)',
	vec3ToVec2YZ: 'Vec3 -> Vec2 (Y,Z)',
	vec3ToColorRgb: 'Vec3 -> RGB',
	vec3ToColorHsv: 'Vec3 -> HSV',
	colorR: 'Color R',
	colorG: 'Color G',
	colorB: 'Color B',
	colorA: 'Color A',
	colorToVec3Rgb: 'RGB -> Vec3',
	colorToVec3Hsv: 'HSV -> Vec3',
	colorToVec2Hs: 'Hue,Sat -> Vec2'
};

export const projectionLabel = (projection: UiParamValueProjection): string =>
	PROJECTION_LABELS[projection];
