// A game description as whole sentences, for the side panels: a long one is cut to
// its opening (premise and story), without release history or the series' family tree.
const ABBREVIATIONS = /\b(Mr|Mrs|Ms|Dr|St|Mt|Vol|No|vs|Jr|Sr|Inc|Co|Ltd|etc|e\.g|i\.e)\./gi;
// Dotted initials and acronyms ("A.D.", "U.S.", "J.U.N.K.E.R.") don't end a sentence.
const ACRONYM = /\b(?:[A-Za-z]\.){2,}/g;
const DOT = "\u0001";

// Release dates, ports, patches, prices and where the game sits in its franchise
// say little about the game itself. Narrow on purpose: a story can "release" a demon.
const TRIVIA = new RegExp(
  [
    String.raw`\b(?:game|version|title|it|edition|port|remake|remaster|sequel)\s+(?:\w+\s+){0,3}released\b`,
    String.raw`\b(?:first|originally|initially|later|finally)\s+released\b`,
    String.raw`\breleased\s+(?:on|in|for|via|exclusively|worldwide|as|outside)\b`,
    String.raw`\bre-released\b`,
    String.raw`\bpatch(?:ed|es)?\b`,
    String.raw`\b(?:the|digital)\s+download\b|\bdownloadable\b`,
    String.raw`\b(?:installment|subseries|franchise|chronologically|remade|ported)\b`,
    String.raw`\$\d`,
    // How the game was made, not what it is.
    String.raw`\b(?:developers|development|in the works|labor of love|community effort)\b`,
  ].join("|"),
  "i",
);

/** The sentences in reading order, without trivia, headers or list items (unless nothing else is left). */
export function sentences(text: string): string[] {
  const all = text
    .split(/\n+/)
    .map((line) => line.trim())
    // "FEATURES", "Key features:" and bullet lists are not the premise.
    .filter((line) => line && !/^[-•*·]/.test(line) && !(line.length < 60 && !/[.!?]["')\]]?$/.test(line)))
    .flatMap((line) =>
      line
        .replace(ABBREVIATIONS, (m) => m.replace(".", DOT))
        .replace(ACRONYM, (m) => m.replaceAll(".", DOT))
        .split(/(?<=[.!?]["')\]]?)\s+(?=["'(\[]?[A-Z0-9])/),
    )
    .map((s) => s.replaceAll(DOT, ".").trim())
    // Only whole sentences: "Including:" before a list, or a line cut off, is not one.
    .filter((s) => /[.!?]["')\]]?$/.test(s));
  const kept = all.filter((s) => !TRIVIA.test(s));
  return kept.length > 0 ? kept : all.length > 0 ? all : [text.trim()];
}

const MIN = 400; // a shortened description says more than this…
const MAX = 800; // …and less than this; one up to MAX shows whole

/** The opening sentences of a long description, trivia skipped, as many as fit under MAX. Null when it shows whole. */
export function lead(text: string): string[] | null {
  if (text.length < MAX) return null;
  const [first, ...rest] = sentences(text);
  const out = [first];
  let size = first.length;
  for (const s of rest) {
    const next = size + 1 + s.length;
    if (next >= MAX) {
      if (size > MIN) break;
      continue; // too long to fit, but still short of MIN: try the next one
    }
    out.push(s);
    size = next;
  }
  return out;
}
