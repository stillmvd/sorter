export const PILE_KEYS = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P"];

export function keyOf(e: KeyboardEvent): string | null {
  if (e.code === "Delete") return "Delete";
  const digit = /^(?:Digit|Numpad)([1-9])$/.exec(e.code);
  if (digit) return digit[1];
  const letter = /^Key([A-Z])$/.exec(e.code);
  return letter ? letter[1] : null;
}

export function isTypingChar(e: KeyboardEvent): boolean {
  return e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey && e.key !== " ";
}
