export function setupLegend() {
  const el = document.querySelector<HTMLElement>("#legend")!;
  el.innerHTML = `
    <div><span class="swatch uninhabitable"></span> 不宜居</div>
    <div><span class="swatch habitable-empty"></span> 宜居无人</div>
    <div><span class="swatch black"></span> 无人（未启用掩码时）</div>
    <div><span class="swatch green"></span> 无感染者（健康/免疫）</div>
    <div><span class="swatch yellow"></span> → <span class="swatch red"></span> 感染占比升高</div>
    <div>明暗表示总人数（对数）</div>
  `;
}
