export function setupLegend() {
  const el = document.querySelector<HTMLElement>("#legend")!;
  el.innerHTML = `
    <div><span class="swatch black"></span> 无人</div>
    <div><span class="swatch green"></span> 无感染者（健康/免疫）</div>
    <div><span class="swatch yellow"></span> → <span class="swatch red"></span> 感染占比升高</div>
    <div>明暗表示总人数（对数）</div>
  `;
}
