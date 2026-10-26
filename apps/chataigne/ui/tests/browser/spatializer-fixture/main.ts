import { mount } from 'svelte';
import Spatializer from './Spatializer.svelte';
const target = document.getElementById('app');
if (!target) throw new Error('Missing Spatializer fixture root');
const fixture = mount(Spatializer, { target });
declare global {
	interface Window {
		spatializerFixture: typeof fixture;
	}
}
window.spatializerFixture = fixture;
