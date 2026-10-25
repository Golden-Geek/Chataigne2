import { mount } from 'svelte';
import DashboardFixture from './DashboardFixture.svelte';
const target = document.getElementById('app');
if (!target) throw new Error('Missing dashboard fixture mount.');
const fixture = mount(DashboardFixture, { target });
declare global {
	interface Window {
		dashboardFixture: typeof fixture;
	}
}
window.dashboardFixture = fixture;
