const X = [-130, 0, 130];

export function Learn({ piles, paused }: { piles: string[]; paused: boolean }) {
  return (
    <div className={`learn ${paused ? "paused" : ""}`} aria-hidden="true">
      {X.map((x, i) => (
        <i key={i} style={{ left: `calc(50% + ${x}px)`, animationDelay: `${i * 1.6}s` }}>
          <span>{piles[i]?.toUpperCase()}</span>
        </i>
      ))}
      {X.map((x, i) => (
        <b
          key={i}
          style={{ ["--x" as string]: `${x}px`, ["--a" as string]: `${120 + ((i * 47) % 90)}deg`, animationDelay: `${i * 1.6}s` }}
        />
      ))}
    </div>
  );
}
