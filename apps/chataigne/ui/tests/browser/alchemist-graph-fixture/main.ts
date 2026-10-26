import { mount } from 'svelte';
import AlchemistGraph from './AlchemistGraph.svelte';
const target = document.getElementById('app');
if (!target) throw new Error('Missing Alchemist graph fixture root');
const fixture = mount(AlchemistGraph, { target });
declare global {
	interface Window {
		alchemistGraphFixture: typeof fixture;
	}
}
window.alchemistGraphFixture = fixture;
