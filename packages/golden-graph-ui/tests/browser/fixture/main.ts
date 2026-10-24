import { mount } from "svelte";
import Fixture from "./Fixture.svelte";

const fixture = mount(Fixture, { target: document.getElementById("app")! });
Object.defineProperty(window, "goldenGraphFixture", { value: fixture });
