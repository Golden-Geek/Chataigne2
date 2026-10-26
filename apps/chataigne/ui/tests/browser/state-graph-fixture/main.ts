import { mount } from 'svelte';
import StateGraph from './StateGraph.svelte';
const target = document.getElementById('app');
if (!target) throw new Error('Missing state graph fixture root');
const fixture = mount(StateGraph, { target });
declare global {
	interface Window {
		stateGraphFixture: typeof fixture;
	}
}
window.stateGraphFixture = fixture;
