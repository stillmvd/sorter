const RUN = 4.8 * 0.72;

export function Scanner({ paused }: { paused: boolean }) {
  return (
    <div className={`scanner ${paused ? "paused" : ""}`} aria-hidden="true">
      {Array.from({ length: 8 }, (_, k) => (
        <i
          key={k}
          style={{ ["--a" as string]: `${120 + ((k * 47) % 90)}deg`, ["--d" as string]: `${(((26 + 58 * k) / 468) * RUN).toFixed(2)}s` }}
        />
      ))}
      <span />
    </div>
  );
}
