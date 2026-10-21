"""Mixes KaneegaBell's intro and menu sounds from CC0 recordings (VSCO 2 CE orchestral samples and
the Free Firearm Sound Library) with ffmpeg. Writes snd/out/<name>.ogg and sounds.js (base64 map)."""
import base64, json, math, os, random, re, struct, subprocess, wave

HERE = os.path.dirname(os.path.abspath(__file__))
FF = os.environ.get("FFMPEG", "ffmpeg")
V = os.path.join(HERE, 'snd', 'vsco')
G = os.path.join(HERE, 'snd', 'guns', 'Prepared SFX Library')
OUT = os.path.join(HERE, 'snd', 'out')
os.makedirs(OUT, exist_ok=True)
SR = 48000
random.seed(7)

def v(name): return os.path.join(V, name + '.wav')

# ---------------------------------------------------------------- onsets
_onsets = {}
def onset(path):
    """Seconds of silence before the sound starts (first point above -45 dB)."""
    if path not in _onsets:
        r = subprocess.run([FF, '-hide_banner', '-i', path, '-af', 'silencedetect=noise=-45dB:d=0.02', '-f', 'null', '-'],
                           capture_output=True, text=True)
        m = re.search(r'silence_start: 0\b.*?silence_end: ([0-9.]+)', r.stderr, re.S)
        _onsets[path] = max(0.0, float(m.group(1)) - 0.004) if m else 0.0
    return _onsets[path]

# ---------------------------------------------------------------- a warm hall reverb (impulse response)
IR = os.path.join(OUT, 'ir.wav')
if not os.path.exists(IR):
    n = int(SR * 2.4); frames = bytearray()
    l = r = 0.0
    for i in range(n):
        t = i / SR
        env = 0.0 if t < 0.012 else math.exp(-t / 0.55)
        # one-pole low-pass on each channel so the tail is warm, not hissy
        l = 0.55 * l + 0.45 * (random.uniform(-1, 1) * env)
        r = 0.55 * r + 0.45 * (random.uniform(-1, 1) * env)
        frames += struct.pack('<hh', int(l * 20000), int(r * 20000))
    with wave.open(IR, 'wb') as w:
        w.setnchannels(2); w.setsampwidth(2); w.setframerate(SR); w.writeframes(bytes(frames))

# ---------------------------------------------------------------- mixer
def ev(path, at, db=0.0, semis=0.0, dur=None, fade=None, fin=0.0, src_at=None, lp=None, hp=None, pan=0.0):
    return dict(path=path, at=at, db=db, semis=semis, dur=dur, fade=fade, fin=fin, src_at=src_at, lp=lp, hp=hp, pan=pan)

def render(name, events, length, wet=0.25, gain_db=0.0, peak=None):
    args = [FF, '-hide_banner', '-loglevel', 'error', '-y']
    chains, labels = [], []
    for i, e in enumerate(events):
        if e['src_at'] is not None: args += ['-ss', '%.4f' % e['src_at']]
        args += ['-i', e['path']]
        f = ['aresample=%d' % SR, 'aformat=sample_fmts=fltp:channel_layouts=stereo']
        if e['src_at'] is None: f.append('silenceremove=start_periods=1:start_threshold=-60dB:detection=peak')
        if e['dur']: f.append('atrim=0:%.4f' % (e['dur'] * (2 ** (e['semis'] / 12)) + 0.01))
        if e['semis']:
            f += ['asetrate=%d' % round(SR * 2 ** (e['semis'] / 12)), 'aresample=%d' % SR]
        if e['hp']: f.append('highpass=f=%d' % e['hp'])
        if e['lp']: f.append('lowpass=f=%d' % e['lp'])
        if e['fin']: f.append('afade=t=in:d=%.3f' % e['fin'])
        if e['dur'] and e['fade']: f.append('afade=t=out:st=%.3f:d=%.3f' % (max(0, e['dur'] - e['fade']), e['fade']))
        if e['pan']: f.append('stereotools=balance_out=%.2f' % e['pan'])
        f.append('volume=%.1fdB' % e['db'])
        f.append('adelay=%d|%d' % (round(e['at'] * 1000), round(e['at'] * 1000)))
        f.append('apad=whole_dur=%.3f' % length)
        chains.append('[%d:a]%s[e%d]' % (i, ','.join(f), i)); labels.append('[e%d]' % i)
    ir = len(events)
    args += ['-i', IR]
    graph = ';'.join(chains)
    graph += ';%samix=inputs=%d:normalize=0:duration=longest,atrim=0:%.3f[mix]' % (''.join(labels), len(events), length)
    graph += ';[mix]asplit[dry][toir];[toir][%d:a]afir=dry=10:wet=10[rev]' % ir
    graph += ';[rev]volume=%.2f[wet];[dry][wet]amix=inputs=2:normalize=0,volume=%.1fdB,afade=t=out:st=%.3f:d=0.25,alimiter=limit=0.89,aresample=%d[out]' % (
        wet, gain_db, max(0, length - 0.25), SR)
    raw = os.path.join(OUT, name + '.wav')
    args += ['-filter_complex', graph, '-map', '[out]', raw]
    r = subprocess.run(args, capture_output=True, text=True)
    if r.returncode: raise SystemExit(name + ': ' + r.stderr[-2000:])
    out = os.path.join(OUT, name + '.ogg')
    if peak is None:
        # intros: the same perceived loudness for all of them (EBU R128, two passes)
        m = subprocess.run([FF, '-hide_banner', '-i', raw, '-af', 'loudnorm=I=-16:TP=-1.5:LRA=11:print_format=json', '-f', 'null', '-'], capture_output=True, text=True).stderr
        j = json.loads(m[m.rindex('{'):m.rindex('}') + 1])
        af = 'loudnorm=I=-16:TP=-1.5:LRA=11:measured_I=%s:measured_TP=%s:measured_LRA=%s:measured_thresh=%s:offset=%s:linear=true,aresample=%d' % (
            j['input_i'], j['input_tp'], j['input_lra'], j['input_thresh'], j['target_offset'], SR)
    else:
        m = subprocess.run([FF, '-hide_banner', '-i', raw, '-af', 'volumedetect', '-f', 'null', '-'], capture_output=True, text=True).stderr
        mx = float(re.search(r'max_volume: ([-0-9.]+) dB', m).group(1))
        af = 'volume=%.1fdB' % (peak - mx)
    r = subprocess.run([FF, '-hide_banner', '-loglevel', 'error', '-y', '-i', raw, '-af', af, '-c:a', 'libopus', '-b:a', '96k', out], capture_output=True, text=True)
    if r.returncode: raise SystemExit(name + ': ' + r.stderr[-2000:])
    return out

# shorthand for pitched samples: (file, its pitch as a MIDI note)
NOTE = {'C': 0, 'D': 2, 'E': 4, 'F': 5, 'G': 7, 'A': 9, 'B': 11}
def midi(n):  # 'C#5' -> 73
    m = re.match(r'([A-G])(#?)(-?\d)', n); return 12 * (int(m.group(3)) + 1) + NOTE[m.group(1)] + (1 if m.group(2) else 0)
GLOCK = {midi(n): v('glock_medium_' + n) for n in ['G4', 'C5', 'G5', 'C6', 'G6', 'C7']}
MARIMBA = {midi(n): v('Marimba_hit_Outrigger_%s_loud_01' % n) for n in ['C4', 'G4', 'B4', 'F5', 'C6']}
PIANO = {21 + 2 * i: v('Player_dyn2_rr1_%03d' % i) for i in range(20, 33, 2)}
def pick(bank, note):
    k = min(bank, key=lambda x: abs(x - note)); return bank[k], note - k
def glock(note, at, db=0, dur=2.5, fade=1.5): p, s = pick(GLOCK, note); return ev(p, at, db, s, dur, fade)
def marimba(note, at, db=0, dur=1.6, fade=.8): p, s = pick(MARIMBA, note); return ev(p, at, db, s, dur, fade)
def piano(note, at, db=0, dur=2.5, fade=1.2): p, s = pick(PIANO, note); return ev(p, at, db, s, dur, fade)

S = {}
# --- Konami (Metal Gear): four short, identical, bright notes; the fourth rings out over a low piano
k = []
for i, t in enumerate([.25, .43, .61, .79]):
    last = i == 3
    k += [piano(midi('E6'), t, -2, 2.2 if last else .13, 1.6 if last else .06),
          glock(midi('E6'), t, -4, 2.2 if last else .13, 1.6 if last else .06)]
k += [piano(midi('E3'), .79, -6, 1.8, 1.4), piano(midi('B3'), .79, -9, 1.8, 1.4),
      ev(v('Timpani2_Hit_v4_rr1_Sum'), .79, -12, 0, 1.6, 1.0)]
S['konami'] = render('konami', k, 2.6, wet=.3)

# --- Rockstar (Red Dead 2): a double-barrel shotgun. The action snaps shut, both barrels fire, it breaks open and
#     is reloaded, then both barrels again; the last blast knocks the emblem over
def shot(path, src, at, db=0):
    e = [ev(path, at, db, 0, 1.1, .6, src_at=src - .01)]
    # each blast comes back off the hills twice, darker each time
    e += [ev(path, at + .34, db - 12, 0, 1.0, .6, src_at=src - .01, lp=1400), ev(path, at + .71, db - 19, 0, 1.0, .6, src_at=src - .01, lp=800)]
    return e
M12 = os.path.join(G, 'Model 12', 'K_22P.wav'); CD = os.path.join(G, 'CD', 'H_21P.wav')
CL = v('Claves1_Hit_v2_rr1_Sum')
def snap(at, db=-10): return [ev(CL, at, db, -11, .1, .05, hp=400), ev(CL, at + .03, db - 4, -6, .08, .04, hp=600)]
r = snap(.5) + shot(M12, .842, .9) + shot(CD, .464, 1.35)
r += snap(1.52, -13) + snap(1.62, -11)
r += shot(M12, 7.445, 1.7) + shot(CD, 3.075, 2.3, 2)
r += [ev(v('susvib_D2_v1_1'), 2.5, -16, 0, 1.35, .8, fin=.6), ev(v('BKCtbss_SusNV_D1_v1_rr1'), 2.5, -14, 0, 1.35, .8, fin=.6)]
S['rockstar'] = render('rockstar', r, 3.9, wet=.12)

# --- Game Boy Advance: a soft tick as each letter lands, a glockenspiel run, then the two bright notes
a = [marimba(midi('C6') + (i % 3) * 2, .05 + .07 * i + .28, -20, .3, .2) for i in range(11)]
a += [ev(v('glock_fx_up_chromatic_fast_01'), 1.30, -8, 0, 1.0, .6)]
a += [glock(midi('G6'), 1.85, -2, 1.0, .5), glock(midi('G5'), 1.85, -8, 1.0, .5), piano(midi('G6'), 1.85, -10, .9, .5),
      glock(midi('C7'), 2.02, 0, 1.2, .9), glock(midi('C6'), 2.02, -7, 1.2, .9), piano(midi('C7'), 2.02, -9, 1.2, .9)]
S['gba'] = render('gba', a, 2.9, wet=.28)

# --- GameCube: a marimba note at every corner the cube turns, then a bright chord as the logo lands
c = [marimba(n, t, -3) for n, t in zip([midi('C4'), midi('E4'), midi('G4'), midi('C5'), midi('E5')], [.05, .37, .69, 1.01, 1.28])]
c += [marimba(midi('C6'), 1.55, -2, 2.0, 1.4), marimba(midi('G5'), 1.55, -5, 2.0, 1.4), glock(midi('C6'), 1.55, -6, 2.0, 1.4),
      piano(midi('C5'), 1.55, -7, 2.0, 1.4), piano(midi('E5'), 1.55, -8, 2.0, 1.4), piano(midi('G5'), 1.55, -9, 2.0, 1.4),
      ev(v('BellTree_Stroke1_v1_Sum'), 1.5, -12, 0, 1.6, 1.0), glock(midi('G6'), 2.05, -10, 1.4, 1.0), glock(midi('C7'), 2.12, -12, 1.4, 1.0)]
S['gamecube'] = render('gamecube', c, 3.6, wet=.25)

# --- PlayStation: a deep swell, a crystal chime as the diamond forms; then a boom on the black screen
p = [ev(v('Timpani1_Roll_v5_rr1_Sum'), 0, -12, 0, 1.3, .5, fin=1.0), ev(v('BKCtbss_SusNV_D1_v1_rr1'), 0, -8, 0, 1.6, .9, fin=.9),
     ev(v('susvib_D2_v1_1'), 0, -10, 0, 1.6, .9, fin=.9), ev(v('MOHorn_sus_A2_v2_1'), .2, -14, 0, 1.4, .8, fin=.8)]
p += [glock(midi('C7'), 1.0, -4, 2.2, 1.6), glock(midi('G6'), 1.03, -6, 2.2, 1.6), glock(midi('C6'), 1.06, -8, 2.2, 1.6),
      ev(v('triangle1_hit_mp'), 1.0, -10, 0, 2.0, 1.4), ev(v('BellTree_Stroke1_v1_Sum'), .95, -10, 0, 2.0, 1.2), piano(midi('C7'), 1.0, -12, 2.0, 1.4)]
p += [ev(v('gong_hit_mp'), 2.95, -6, 0, 1.85, 1.2), ev(v('bdrum_fff_1'), 2.95, -6, 0, 1.6, 1.0), ev(v('Timpani2_Hit_v4_rr1_Sum'), 2.95, -6, 0, 1.6, 1.0),
      ev(v('MOHorn_sus_D2_v2_1'), 3.0, -10, 0, 1.75, 1.0, fin=.2), ev(v('MOHorn_sus_A2_v2_1'), 3.0, -12, 0, 1.75, 1.0, fin=.2)]
S['ps1'] = render('ps1', p, 4.8, wet=.35)

# --- PlayStation 2: a low drone, glass tinkles in the haze, then a big string chord as the name appears
q = [ev(v('trem_D2_v1_1'), 0, -14, 0, 2.6, 1.0, fin=1.2), ev(v('BKCtbss_SusNV_D1_v1_rr1'), 0, -12, 0, 2.6, 1.0, fin=1.2)]
for t, n in [(.35, 'C7'), (.7, 'G6'), (1.0, 'C7'), (1.35, 'G6'), (1.6, 'C6'), (1.9, 'G6'), (2.2, 'C7')]:
    q.append(glock(midi(n) + random.choice([0, 2, -3]), t, -16, 1.5, 1.2))
q += [ev(v('susp_hit_softmall_roll2_cresc'), 1.0, -14, 0, 1.6, .2, fin=.6), ev(v('triangle1_hit_mp'), 2.35, -12, 0, 1.7, 1.2)]
for f_, db in [('VlnEns_susVib_D5_v1', -14), ('VlnEns_susVib_A3_v1', -14), ('ViolaEns_susvib_D3_v1_1', -13), ('susvib_A2_v1_1', -12),
               ('susvib_D2_v1_1', -11), ('MOHorn_sus_D2_v2_1', -14)]:
    q.append(ev(v(f_), 2.2, db, 0, 1.9, 1.1, fin=.5))
S['ps2'] = render('ps2', q, 4.1, wet=.4)

# --- PlayStation 3: a slow orchestral swell in D that peaks as the name appears, a shimmer on top
z = []
for f_, db in [('VlnEns_susVib_D5_v1', -15), ('VlnEns_susVib_A3_v1', -15), ('VlnEns_susVib_F#3_v1', -16), ('ViolaEns_susvib_D3_v1_1', -14),
               ('susvib_A2_v1_1', -13), ('susvib_D2_v1_1', -12), ('BKCtbss_SusNV_D1_v1_rr1', -12), ('MOHorn_sus_D2_v2_1', -16)]:
    z.append(ev(v(f_), .1, db, 0, 4.2, 1.6, fin=2.5))
z += [glock(midi('D7'), 2.75, -10, 1.6, 1.2), glock(midi('A6'), 2.8, -12, 1.6, 1.2), ev(v('triangle1_hit_mp'), 2.75, -14, 0, 1.6, 1.2),
      ev(v('BellTree_Stroke1_v1_Sum'), 2.55, -14, 0, 1.8, 1.2), ev(v('susp_hit_softmall_roll2_cresc'), 1.2, -18, 0, 1.6, .3, fin=.8)]
S['ps3'] = render('ps3', z, 4.4, wet=.4)

# --- PSP: a soft string swell under the line of light, then a bright chime as the name flashes in
y = [ev(v('VlnEns_susVib_A3_v1'), .1, -18, 0, 2.2, 1.0, fin=1.0), ev(v('ViolaEns_susvib_D3_v1_1'), .1, -17, 0, 2.2, 1.0, fin=1.0), ev(v('susvib_D2_v1_1'), .1, -16, 0, 2.2, 1.0, fin=1.0)]
y += [glock(midi('C7'), 2.3, -4, 1.4, 1.0), glock(midi('G6'), 2.32, -8, 1.4, 1.0), piano(midi('C7'), 2.3, -10, 1.4, 1.0), ev(v('triangle1_hit_mp'), 2.3, -12, 0, 1.4, 1.0)]
S['psp'] = render('psp', y, 3.8, wet=.35)

# --- Xbox: an eerie tremolo builds, a cymbal swell, then a crash and a heroic D minor chord
x = [ev(v('trem_D2_v1_1'), 0, -14, 0, 2.4, .3, fin=1.6), ev(v('trem_A2_v1_1'), .3, -16, 0, 2.1, .3, fin=1.4),
     ev(v('VlnEns_susVib_D5_v1'), .6, -22, 0, 1.8, .3, fin=1.2), ev(v('susp_hit_softmall_roll2_cresc'), .2, -10, 0, 2.15, .1, fin=.4)]
x += [ev(v('crash_hit_ff_loose'), 2.3, -8, 0, 1.5, 1.0), ev(v('gong_hit_mp'), 2.3, -10, 0, 1.5, 1.0), ev(v('bdrum_fff_1'), 2.3, -4, 0, 1.4, .9),
      ev(v('Timpani1_Hit_v3_rr1_Sum'), 2.3, -6, 0, 1.4, .9)]
for f_, db in [('MOHorn_sus_D2_v2_1', -8), ('MOHorn_sus_F2_v2_1', -10), ('MOHorn_sus_A2_v2_1', -10), ('susvib_D2_v1_1', -10), ('VlnEns_susVib_A3_v1', -14)]:
    x.append(ev(v(f_), 2.3, db, 0, 1.5, 1.0))
S['xbox'] = render('xbox', x, 3.8, wet=.3)

# --- Switch: the snap, a short hard click and a lighter one just after
w = [ev(v('Claves1_Hit_v2_rr1_Sum'), .58, -3, 7, .09, .04, hp=900), ev(v('Claves1_Hit_v2_rr1_Sum'), .63, -12, 12, .06, .03, hp=1500)]
S['switch'] = render('switch', w, 1.2, wet=.05, peak=-3)

# --- menu and game-case sounds
S['move'] = render('move', [glock(midi('C7'), 0, -14, .25, .2)], .35, wet=.1, peak=-16)
S['select'] = render('select', [marimba(midi('G5'), 0, -6, .4, .3), marimba(midi('C6'), .06, -6, .5, .4)], .6, wet=.15, peak=-9)
S['caseOpen'] = render('caseOpen', [ev(v('Claves1_Hit_v2_rr1_Sum'), 0, -10, -4, .08, .04, hp=700), ev(v('Claves1_Hit_v2_rr1_Sum'), .05, -14, 3, .07, .04, hp=900)], .3, wet=.05, peak=-8)
S['discSpin'] = render('discSpin', [ev(v('glock_fx_up_pentatonic_med_01'), 0, -14, 0, 1.6, .8), ev(v('BellTree_Stroke1_v1_Sum'), .1, -16, 0, 1.6, 1.0)], 1.8, wet=.3, peak=-14)
S['cartClunk'] = render('cartClunk', [ev(v('bdrum_fff_1'), 0, -12, 4, .25, .15, lp=900), ev(v('Claves1_Hit_v2_rr1_Sum'), 0, -10, -8, .1, .05)], .4, wet=.05, peak=-6)
S['powerOn'] = render('powerOn', [glock(midi('G6'), 0, -10, .6, .4), glock(midi('C7'), .1, -10, .8, .6)], 1.0, wet=.2, peak=-11)
S['loading'] = render('loading', [marimba(midi('C5'), 0, -12, 1.2, .8), marimba(midi('G5'), .1, -13, 1.2, .8), marimba(midi('C6'), .2, -14, 1.2, .8)], 1.4, wet=.3, peak=-13)

# --- Menu set D, "Aero": glassy, watery, airy (Frutiger Aero). Each one clearly its own:
#     Move = a single bright water droplet; Select = a rising glass chord with sparkle; Back = two notes falling into a soft bloop
K = os.path.join(HERE, 'snd', 'kenney', 'interface', 'Audio')
def kn(n):
    p_ = os.path.join(K, n + '.ogg')
    return p_ if os.path.exists(p_) else os.path.join(HERE, 'snd', 'kenney', 'ui', 'Audio', n + '.ogg')
S['menu_d_move'] = render('menu_d_move', [ev(kn('drop_002'), 0, -4, 5, .3, .2), glock(midi('E7'), .012, -14, .5, .4)], .6, wet=.35, peak=-16)
S['menu_d_select'] = render('menu_d_select', [ev(kn('glass_002'), 0, -6, 0, .6, .4), glock(midi('C7'), .0, -6, 1.2, .9), glock(midi('E7'), .035, -7, 1.2, .9),
    glock(midi('G7'), .07, -8, 1.2, .9), piano(midi('C7'), 0, -14, 1.0, .8), ev(v('BellTree_Stroke1_v1_Sum'), .02, -18, 0, 1.1, .8)], 1.4, wet=.45, peak=-9)
S['menu_d_back'] = render('menu_d_back', [glock(midi('G6'), 0, -8, .8, .6), glock(midi('C6'), .09, -8, .9, .7), ev(kn('drop_003'), .1, -9, -5, .35, .25)], 1.1, wet=.45, peak=-11)

# --- Starting a game: three sets to compare. Disc games: caseOpen, discSpin, loading (the hit once the disc is up to speed).
#     Cartridges: cartSlide, cartClunk, powerOn.
KB_ = {
  'a': {  # Cinematic: timpani, cymbal swell, a horn chord as the game starts
    'caseOpen': ([ev(kn('open_002'), 0, -8), ev(v('Timpani2_Hit_v4_rr1_Sum'), 0, -20, 0, .6, .4)], .7, .1),
    'discSpin': ([ev(v('susp_hit_softmall_roll2_cresc'), 0, -10, 0, 1.3, .2, fin=.4), ev(v('Timpani1_Roll_v5_rr1_Sum'), 0, -16, 0, 1.3, .3, fin=.9)], 1.4, .25),
    'loading': ([ev(v('crash_hit_ff_loose'), .95, -14, 0, 1.4, 1.0), ev(v('bdrum_fff_1'), .95, -8, 0, 1.0, .7), ev(v('MOHorn_sus_D2_v2_1'), .95, -10, 0, 1.4, 1.0),
                 ev(v('MOHorn_sus_A2_v2_1'), .95, -12, 0, 1.4, 1.0), ev(v('susvib_D2_v1_1'), .95, -12, 0, 1.4, 1.0)], 2.5, .35),
    'cartSlide': ([ev(kn('scroll_003'), 0, -10), ev(kn('scratch_001'), .05, -16)], .6, .1),
    'cartClunk': ([ev(v('bdrum_fff_1'), 0, -6, 0, .6, .4), ev(v('Timpani1_Hit_v3_rr1_Sum'), 0, -10, 0, .8, .5), ev(kn('close_002'), 0, -8)], .9, .2),
    'powerOn': ([ev(v('crash_hit_ff_loose'), .05, -18, 0, 1.2, .9), ev(v('MOHorn_sus_D2_v2_1'), .05, -12, 0, 1.2, .9), glock(midi('D7'), .05, -14, 1.0, .8)], 1.4, .35),
  },
  'b': {  # Aero: glass, water and chimes
    'caseOpen': ([ev(kn('glass_001'), 0, -6), ev(kn('drop_001'), .03, -10, 3)], .6, .3),
    'discSpin': ([ev(v('glock_fx_up_pentatonic_med_01'), 0, -10, 0, 1.4, .5), ev(v('VlnEns_susVib_E4_v1'), 0, -18, 0, 1.4, .6, fin=.8), ev(v('BellTree_Stroke1_v1_Sum'), .3, -16, 0, 1.2, .8)], 1.5, .45),
    'loading': ([glock(midi('C7'), .95, -6, 1.3, 1.0), glock(midi('E7'), .98, -7, 1.3, 1.0), glock(midi('G7'), 1.01, -8, 1.3, 1.0), piano(midi('C6'), .95, -12, 1.3, 1.0),
                 piano(midi('E6'), .95, -13, 1.3, 1.0), ev(v('triangle1_hit_mp'), .95, -12, 0, 1.3, 1.0)], 2.4, .45),
    'cartSlide': ([ev(kn('maximize_002'), 0, -8)], .6, .3),
    'cartClunk': ([ev(kn('drop_004'), 0, -6, -7), ev(kn('close_001'), 0, -10)], .6, .3),
    'powerOn': ([ev(kn('glass_003'), 0, -8), glock(midi('G6'), .05, -8, .9, .7), glock(midi('C7'), .12, -8, 1.0, .8), glock(midi('E7'), .19, -10, 1.0, .8)], 1.3, .45),
  },
  'c': {  # Clicky: real buttons, latches and ticks, like hardware
    'caseOpen': ([ev(kn('click_002'), 0, -6), ev(kn('switch_004'), .05, -8), ev(v('Claves1_Hit_v2_rr1_Sum'), 0, -14, -10, .1, .05)], .4, .05),
    'discSpin': ([ev(kn('tick_001'), t, -10 - i * .4) for i, t in enumerate([0, .22, .4, .55, .67, .77, .85, .92, .98, 1.03, 1.08, 1.12, 1.16, 1.2])], 1.45, .08),
    'loading': ([ev(kn('confirmation_002'), .95, -6), ev(kn('bong_001'), 1.0, -10)], 2.2, .15),
    'cartSlide': ([ev(kn('scratch_003'), 0, -8)], .5, .05),
    'cartClunk': ([ev(kn('switch12' if os.path.exists(os.path.join(HERE, 'snd', 'kenney', 'ui', 'Audio', 'switch12.ogg')) else 'switch_006'), 0, -4), ev(kn('click_005'), .02, -8), ev(v('bdrum_fff_1'), 0, -16, 7, .3, .2)], .5, .05),
    'powerOn': ([ev(kn('toggle_002'), 0, -6), ev(kn('tick_004'), .08, -8), ev(kn('confirmation_004'), .15, -8)], 1.0, .1),
  },
}
PEAKS = {'caseOpen': -8, 'discSpin': -10, 'loading': -6, 'cartSlide': -10, 'cartClunk': -5, 'powerOn': -8}
for k, set_ in KB_.items():
    for role, (evs, length, wet) in set_.items():
        S['boot_%s_%s' % (k, role)] = render('boot_%s_%s' % (k, role), evs, length, wet=wet, peak=PEAKS[role])

js = 'var SOUNDS = ' + json.dumps({k: 'data:audio/ogg;base64,' + base64.b64encode(open(p_, 'rb').read()).decode() for k, p_ in S.items()}) + ';\n'
open(os.path.join(HERE, 'sounds.js'), 'w').write(js)
print({k: os.path.getsize(p_) // 1000 for k, p_ in S.items()}, 'sounds.js', len(js) // 1000, 'KB')
