import { invoke } from "@tauri-apps/api/core";

let wired = false;

export function setupControls(stats: Record<string, unknown>) {
  const state = String(stats.run_state ?? "idle");
  const day = Number(stats.day ?? 0);
  const maxDays = 10000;

  const btnStart = document.querySelector<HTMLButtonElement>("#btn-start")!;
  const btnPause = document.querySelector<HTMLButtonElement>("#btn-pause")!;
  const btnResume = document.querySelector<HTMLButtonElement>("#btn-resume")!;
  const btnStop = document.querySelector<HTMLButtonElement>("#btn-stop")!;
  const speed = document.querySelector<HTMLInputElement>("#speed")!;
  const err = document.querySelector<HTMLElement>("#error")!;

  btnStart.disabled = state !== "idle";
  btnPause.disabled = state !== "running";
  btnResume.disabled = state !== "paused" || day >= maxDays;
  btnStop.disabled = state === "idle" || state === "generating";

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
    btnStop.addEventListener("click", () => void invoke("stop"));
    void invoke("set_speed", { speed: Number(speed.value) });
    speed.addEventListener("input", () => {
      const v = Number(speed.value);
      void invoke("set_speed", { speed: v < 0 ? 0 : v });
    });
  }
}
