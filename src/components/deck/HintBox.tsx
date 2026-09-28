import type { Hints, Pile } from "../../lib/ipc";
import { cardsWord, onCardsWord } from "../../lib/plural";
import { Button } from "../ui/Button";

const pct = (s: number) => `${Math.round(s * 100)}%`;

export function HintBox({ hints, piles, onPlace }: { hints: Hints; piles: Pile[]; onPlace: (pile: Pile) => void }) {
  if (!hints.enabled) return null;
  const [first, second] = hints.hints.map((h) => ({ ...h, pile: piles.find((p) => p.id === h.pileId) })).filter((h) => h.pile);
  const learning = `учится на ${onCardsWord(hints.examples)}`;
  return (
    <div className="flex min-h-[76px] items-center gap-3.5 rounded-[20px] bg-raised px-4 py-3.5">
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <div className="flex items-center gap-2 text-xs font-medium text-dim">
          <span className="h-1.5 w-1.5 rounded-full border-[1.5px] border-fg" />
          {first ? `Просится в стопку · ${learning}` : !hints.ready ? "Смотрю на кадры…" : hints.examples < 3 ? `Подсказка учится · ${cardsWord(hints.examples)} в стопках` : `Не похоже ни на одну стопку · ${learning}`}
        </div>
        {first?.pile && (
          <div className="flex min-w-0 items-baseline gap-2.5">
            <span className="truncate text-[28px] font-bold tracking-[-0.02em]">{first.pile.name}</span>
            <span className="text-[13px] font-bold">{pct(first.score)}</span>
            {second?.pile && (
              <span className="truncate text-[13px] text-dim">
                дальше {second.pile.name} {pct(second.score)}
              </span>
            )}
          </div>
        )}
      </div>
      {first?.pile && (
        <Button variant="primary" size={44} hotkey="Enter" onClick={() => onPlace(first.pile!)}>
          Положить
        </Button>
      )}
    </div>
  );
}
