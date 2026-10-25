import "./styles.css";
import { invoke } from "@tauri-apps/api/core";
import { setupControls } from "./ui/controls";
import { setupStatsPanel } from "./ui/statsPanel";
import { setupLegend } from "./ui/legend";
import { MapRenderer } from "./renderer";

const canvas = document.querySelector<HTMLCanvasElement>("#map")!;
const renderer = new MapRenderer(canvas);

let viewDirty = true;
let lastStats: Record<string, unknown> = { run_state: "idle" };

async function refreshStats() {
  lastStats = await invoke<Record<string, unknown>>("get_stats");
  setupStatsPanel(lastStats);
  setupControls(lastStats);
  viewDirty = true;
}

async function refreshView() {
  if (!viewDirty) return;
  const view = await invoke<number[]>("get_view");
  renderer.drawFromBuffer(new Uint8Array(view));
  viewDirty = false;
}

setupLegend();
setupControls({ run_state: "idle" });

let hoverTimer: ReturnType<typeof setTimeout> | undefined;
canvas.addEventListener("mousemove", (ev) => {
  const me = ev as MouseEvent;
  if (hoverTimer) clearTimeout(hoverTimer);
  hoverTimer = setTimeout(() => {
    void (async () => {
      const rect = canvas.getBoundingClientRect();
      const x = me.clientX - rect.left;
      const y = me.clientY - rect.top;
      const info = renderer.cellAtPixel(x, y);
      const tip = document.querySelector<HTMLElement>("#hover-tip")!;
      if (!info) {
        tip.textContent = "";
        return;
      }
      const counts = await invoke<{ healthy: number; infected: number; immune: number }>(
        "query_cell",
        { q: info.q, r: info.r },
      );
      tip.textContent = `(${info.q}, ${info.r}) 健康 ${counts.healthy} · 感染 ${counts.infected} · 免疫 ${counts.immune}`;
    })();
  }, 80);
});

setInterval(() => void refreshStats(), 100);
setInterval(() => void refreshView(), 250);

void refreshStats();
void refreshView();
