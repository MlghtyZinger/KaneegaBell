// Run with: npm test (Node's built-in runner; Node strips the TypeScript types).
import { test } from "node:test";
import assert from "node:assert/strict";
import { lead, sentences } from "../src/state/summary.ts";

const pad = (s: string) => s + " " + "Filler sentence about the game to make it long enough to cut. ".repeat(20);

test("dotted acronyms and dates don't end a sentence", () => {
  assert.deepEqual(sentences("A.D. 2047. Neo Kobe City, Japan. J.U.N.K.E.R.s has been formed."), [
    "A.D. 2047.",
    "Neo Kobe City, Japan.",
    "J.U.N.K.E.R.s has been formed.",
  ]);
});

test("release history and franchise trivia are skipped, story is kept", () => {
  const s = sentences(
    "Snake Eater 3D is a stealth game. It was released on February 21, 2012 in North America. " +
      "In 1964, Naked Snake infiltrates the Soviet jungle to rescue a scientist. " +
      "It is the third installment in the Persona series. A patch fixed a freeze.",
  );
  assert.deepEqual(s, ["Snake Eater 3D is a stealth game.", "In 1964, Naked Snake infiltrates the Soviet jungle to rescue a scientist."]);
});

test("a story use of 'released' is not trivia", () => {
  assert.equal(sentences("An ancient demon is released from its seal. You must stop it.").length, 2);
});

test("headers and bullet lists are not the premise", () => {
  assert.deepEqual(sentences("A haunted house adventure.\nFEATURES\n- Vacuum ghosts\n- Explore 3D rooms"), ["A haunted house adventure."]);
});

test("short descriptions show whole", () => {
  assert.equal(lead("A short description. Two sentences."), null);
});

test("a cut description says more than 400 and less than 800 characters", () => {
  const out = lead(pad("The premise is here.")!)!;
  const size = out.join(" ").length;
  assert.equal(out[0], "The premise is here.");
  assert.ok(size > 400 && size < 800, `size ${size}`);
});

test("a sentence too long to fit is skipped while still under 400", () => {
  const huge = "Huge " + "word ".repeat(200).trim() + ".";
  const out = lead("Short opening. " + huge + " " + "A normal sentence about the story of the game. ".repeat(20))!;
  assert.ok(!out.includes(huge));
  assert.ok(out.join(" ").length > 400);
});

test("descriptions under 800 characters show whole", () => {
  assert.equal(lead("A sentence about the game. ".repeat(25)), null); // 675 characters
});

test("a lead-in to a list and development history are left out", () => {
  assert.deepEqual(
    sentences(
      "A boy saves the planet from aliens. It was created by over 30 developers. It is smoother to play today. Including:\n\u2022 Run button",
    ),
    ["A boy saves the planet from aliens.", "It is smoother to play today."],
  );
});
