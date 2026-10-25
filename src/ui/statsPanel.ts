function formatRunState(s: string): string {
  if (s === "generating") return "正在生成世界…";
  if (s === "running") return "运行中";
  if (s === "paused") return "已暂停";
  return "未开始";
}

export function setupStatsPanel(stats: Record<string, unknown>) {
  const el = document.querySelector<HTMLElement>("#stats")!;
  const lines = [
    `天数: ${stats.day ?? 0}`,
    `存活: ${stats.alive ?? 0}`,
    `健康: ${stats.healthy ?? 0} · 感染: ${stats.infected ?? 0} · 免疫: ${stats.immune ?? 0}`,
    `当日新增感染: ${stats.new_infections ?? 0} · 免疫: ${stats.new_immune ?? 0}`,
    `病毒死亡(当日/累计): ${stats.new_virus_deaths ?? 0} / ${stats.total_virus_deaths ?? 0}`,
    `自然死亡(当日/累计): ${stats.new_natural_deaths ?? 0} / ${stats.total_natural_deaths ?? 0}`,
    `出生(当日/累计): ${stats.new_births ?? 0} / ${stats.total_births ?? 0}`,
    `状态: ${formatRunState(String(stats.run_state ?? "idle"))}`,
  ];
  el.textContent = lines.join("\n");
}
