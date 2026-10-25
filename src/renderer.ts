import { invoke } from "@tauri-apps/api/core";

export type Camera = {
  center_q: number;
  center_r: number;
  cells_per_pixel: number;
};

export class MapRenderer {
  private camera: Camera = { center_q: 500, center_r: 500, cells_per_pixel: 2 };
  private viewW = 512;
  private viewH = 512;

  constructor(private canvas: HTMLCanvasElement) {
    canvas.width = this.viewW;
    canvas.height = this.viewH;
    canvas.addEventListener("wheel", (ev) => this.onWheel(ev), { passive: false });
    let dragging = false;
    let lastX = 0;
    let lastY = 0;
    canvas.addEventListener("mousedown", (ev) => {
      dragging = true;
      lastX = ev.clientX;
      lastY = ev.clientY;
    });
    window.addEventListener("mouseup", () => {
      dragging = false;
    });
    window.addEventListener("mousemove", (ev) => {
      if (!dragging) return;
      const dx = ev.clientX - lastX;
      const dy = ev.clientY - lastY;
      lastX = ev.clientX;
      lastY = ev.clientY;
      this.camera.center_q -= dx * this.camera.cells_per_pixel;
      this.camera.center_r -= dy * this.camera.cells_per_pixel;
      void invoke("set_camera", { camera: this.camera });
    });
  }

  private onWheel(ev: WheelEvent) {
    ev.preventDefault();
    const factor = ev.deltaY > 0 ? 1.1 : 0.9;
    this.camera.cells_per_pixel = Math.min(500, Math.max(0.25, this.camera.cells_per_pixel * factor));
    void invoke("set_camera", { camera: this.camera });
  }

  drawFromBuffer(buf: Uint8Array) {
    if (buf.length < 4) return;
    const w = buf[0] | (buf[1] << 8);
    const h = buf[2] | (buf[3] << 8);
    this.viewW = w;
    this.viewH = h;
    this.canvas.width = w;
    this.canvas.height = h;
    const ctx = this.canvas.getContext("2d")!;
    const image = ctx.createImageData(w, h);
    image.data.set(buf.subarray(4, 4 + w * h * 4));
    ctx.putImageData(image, 0, 0);
  }

  cellAtPixel(px: number, py: number): { q: number; r: number } | null {
    const q = Math.floor(this.camera.center_q + (px - this.viewW / 2) * this.camera.cells_per_pixel);
    const r = Math.floor(this.camera.center_r + (py - this.viewH / 2) * this.camera.cells_per_pixel);
    return { q, r };
  }
}
