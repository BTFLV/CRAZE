import { mount } from "svelte";
import "./app.css";
import "uplot/dist/uPlot.min.css";
import App from "./App.svelte";
import { installDevBridge } from "./lib/devbridge";

if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) installDevBridge();

const app = mount(App, { target: document.getElementById("app")! });

export default app;
