import { fx } from "../state/fx";
import { session } from "../state/session";
import introUrl from "../assets/ps2/intro-sync.ogg";
import loopUrl from "../assets/ps2/ambience-loop.ogg";
import bridgeUrl from "../assets/ps2/intro-bridge.ogg";

// One audio clock and gain across the approved 8.8s intro and its menu.
let context: AudioContext | undefined, gain: GainNode | undefined;
let decoded: Promise<AudioBuffer[]> | undefined;
let sources: AudioBufferSourceNode[] = [];
let inIntro = false, menu = false;
export async function preparePs2Audio() {
  context ??= new AudioContext();
  if (!gain) { gain = context.createGain(); gain.connect(context.destination); }
  if (context.state === "suspended") await context.resume().catch(() => {});
  decoded ??= Promise.all([introUrl, loopUrl, bridgeUrl].map(async url => context!.decodeAudioData(await (await fetch(url)).arrayBuffer())));
  return decoded;
}
function stop() { for (const source of sources) { source.stop(); source.disconnect(); } sources = []; }
function start(buffer: AudioBuffer, when: number, loop = false) {
  const source = context!.createBufferSource(); source.buffer = buffer; source.loop = loop;
  source.connect(gain!); source.start(when); sources.push(source);
}
export function syncPs2Audio(active: boolean) {
  menu = active;
  if (!gain || !context) return;
  gain.gain.value = (menu || inIntro) && fx.soundOn() && !session.running() && !document.hidden ? fx.volume() * .55 : 0;
  if (!menu && !inIntro) stop();
  if (menu && !sources.length) void preparePs2Audio().then(([, loop]) => {
    if (menu && !sources.length) start(loop, context!.currentTime, true);
  }).catch(() => {});
}
export async function startPs2Intro() {
  const [intro, loop] = await preparePs2Audio();
  stop(); inIntro = true; const now = context!.currentTime;
  start(intro, now); start(loop, now + intro.duration, true); syncPs2Audio(menu);
}
export function endPs2Intro(natural: boolean) {
  inIntro = false;
  if (!natural) {
    stop();
    void preparePs2Audio().then(([, loop, bridge]) => {
      if (!menu) return;
      const now = context!.currentTime; start(bridge, now); start(loop, now + bridge.duration, true);
    }).catch(() => {});
  }
  // Keep the scheduled recording alive during the intro overlay's 300ms reveal.
  if (natural) menu = fx.tv() && fx.get("tvMenu") === "ps2";
  syncPs2Audio(menu);
}
