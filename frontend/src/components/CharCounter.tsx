interface CharCounterProps {
  length: number;
  max: number;
}

export function CharCounter({ length, max }: CharCounterProps) {
  const overLimit = length > max;
  return (
    <span
      className={overLimit ? "char-counter char-counter--over" : "char-counter"}
    >
      {length}/{max}
    </span>
  );
}
