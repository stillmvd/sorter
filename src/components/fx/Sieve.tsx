const COLS = 10;
const PAIRS: [number, number, number][] = [
  [3, 27, 0],
  [41, 16, 2],
  [9, 35, 4],
];
const at = (i: number) => [(i % COLS) * 52 + 20, Math.floor(i / COLS) * 68 + 28];
const MATCH = new Map(PAIRS.flatMap(([a, b, d]) => [[a, d], [b, d]] as [number, number][]));

export function Sieve({ paused }: { paused: boolean }) {
  return (
    <div className={`sieve ${paused ? "paused" : ""}`} aria-hidden="true">
      {Array.from({ length: 50 }, (_, i) => {
        const d = MATCH.get(i);
        return d === undefined ? (
          <i key={i} style={{ animationDelay: `${((i % COLS) * 0.12 + Math.floor(i / COLS) * 0.06).toFixed(2)}s` }} />
        ) : (
          <i key={i} className="match" style={{ animationDelay: `${d}s` }} />
        );
      })}
      <svg viewBox="0 0 508 328">
        {PAIRS.map(([a, b, d]) => {
          const [x1, y1] = at(a);
          const [x2, y2] = at(b);
          const len = Math.round(Math.hypot(x2 - x1, y2 - y1) * 1.35);
          return (
            <path
              key={a}
              d={`M${x1} ${y1}Q${(x1 + x2) / 2} ${Math.min(y1, y2) - 40} ${x2} ${y2}`}
              style={{ animationDelay: `${d}s`, strokeDasharray: len, ["--len" as string]: len }}
            />
          );
        })}
      </svg>
    </div>
  );
}
