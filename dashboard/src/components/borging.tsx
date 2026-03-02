import "./borging.css";

export function BorgingIndicator() {
  const letters = "BORGING...".split("");
  return (
    <div className="flex items-start py-1">
      <span className="borging-text flex">
        {letters.map((ch, i) => (
          <span
            key={i}
            className="borging-letter"
            style={{ animationDelay: `${i * 0.08}s` }}
          >
            {ch}
          </span>
        ))}
      </span>
    </div>
  );
}
