import { invoke } from "@tauri-apps/api/core";

let wired = false;

export function setupControls(stats: Record<string, unknown>) {
  const state = String(stats.run_state ?? "idle");
  const day = Number(stats.day ?? 0);
  const maxDays = 10000;

  const btnStart = document.querySelector<HTMLButtonElement>("#btn-start")!;
  const btnPause = document.querySelector<HTMLButtonElement>("#btn-pause")!;
  const btnResume = document.querySelector<HTMLButtonElement>("#btn-resume")!;
  const btnReset = document.querySelector<HTMLButtonElement>("#btn-reset")!;
  const speed = document.querySelector<HTMLInputElement>("#speed")!;
  const err = document.querySelector<HTMLElement>("#error")!;

  btnStart.disabled = state !== "idle";
  btnPause.disabled = state !== "running";
  btnResume.disabled = state !== "paused" || day >= maxDays;
  btnReset.disabled = state === "idle";

  if (stats.error) {
    err.textContent = String(stats.error);
  } else {
    err.textContent = "";
  }

  if (!wired) {
    wired = true;
    btnStart.addEventListener("click", () => void invoke("start"));
    btnPause.addEventListener("click", () => void invoke("pause"));
    btnResume.addEventListener("click", () => void invoke("resume"));
    btnReset.addEventListener("click", () => void invoke("reset"));
    speed.addEventListener("input", () => {
      const v = Number(speed.value);
      void invoke("set_speed", { speed: v < 0 ? 0 : v });
    });
  }
}
