export type Sound = "place" | "trash" | "undo" | "error" | "new_pile" | "defer" | "batch" | "empty";

const files = import.meta.glob("../sounds/*.{wav,mp3,ogg}", {
  eager: true,
  query: "?url",
  import: "default",
}) as Record<string, string>;

const GAIN: Partial<Record<Sound, number>> = { trash: 6, defer: 2.5 };

const buffers = new Map<string, AudioBuffer>();
let ctx: AudioContext | null = null;

async function load() {
  ctx = new AudioContext();
  await Promise.all(
    Object.entries(files).map(async ([path, url]) => {
      const name = path.split("/").pop()!.replace(/\.\w+$/, "");
      const data = await (await fetch(url)).arrayBuffer();
      buffers.set(name, await ctx!.decodeAudioData(data));
    }),
  );
}

void load().catch(() => undefined);

export function play(sound: Sound, volume = 0.6) {
  const buffer = buffers.get(sound);
  if (!ctx || !buffer) return;
  if (ctx.state === "suspended") void ctx.resume();
  const source = ctx.createBufferSource();
  const gain = ctx.createGain();
  source.buffer = buffer;
  source.playbackRate.value = 0.97 + Math.random() * 0.06;
  gain.gain.value = volume * (GAIN[sound] ?? 1);
  source.connect(gain).connect(ctx.destination);
  source.start();
}
