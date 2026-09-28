import { Volume1, Volume2, VolumeX } from "lucide-react";

export function VolumeRow({
  muted,
  volume,
  onMute,
  onVolume,
}: {
  muted: boolean;
  volume: number;
  onMute: () => void;
  onVolume: (v: number) => void;
}) {
  const Icon = muted || volume === 0 ? VolumeX : volume < 0.5 ? Volume1 : Volume2;
  const pct = Math.round((muted ? 0 : volume) * 100);
  return (
    <div className="flex items-center gap-3">
      <button
        type="button"
        aria-label={muted ? "Включить звук" : "Выключить звук"}
        title={muted ? "Включить звук — M" : "Выключить звук — M"}
        onClick={onMute}
        className="grid h-9 w-9 shrink-0 place-items-center rounded-full bg-raised text-fg transition-colors duration-200 ease-trail hover:bg-strong"
      >
        <Icon className="h-4 w-4" strokeWidth={1.5} />
      </button>
      <input
        type="range"
        min={0}
        max={1}
        step={0.05}
        value={muted ? 0 : volume}
        aria-label="Громкость видео"
        onChange={(e) => onVolume(Number(e.target.value))}
        className="h-1.5 w-40 cursor-pointer appearance-none rounded-full bg-strong [&::-webkit-slider-thumb]:h-4 [&::-webkit-slider-thumb]:w-4 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-fg"
        style={{ background: `linear-gradient(to right, var(--fg) ${pct}%, var(--strong) ${pct}%)` }}
      />
      <span className="w-9 text-xs font-medium text-dim">{pct}%</span>
    </div>
  );
}
