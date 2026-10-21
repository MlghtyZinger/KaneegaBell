"""Round 3: clean, system-menu style sounds (soft pings, clicks with a small body, airy swooshes, a chord bloom).
Tones are generated with ffmpeg (pure sines with soft envelopes), clicks are Kenney's CC0 recordings; everything goes
through the same hall reverb as the other KB sounds. Writes picker.json for kb-picker.html."""
import base64, json, os, subprocess
HERE = os.path.dirname(os.path.abspath(__file__))
src = open(os.path.join(HERE, 'build_sounds.py'), encoding='utf8').read()
exec(src[:src.index('S = {}')])
GEN = os.path.join(HERE, 'snd', 'gen'); os.makedirs(GEN, exist_ok=True)
K1 = os.path.join(HERE, 'snd', 'kenney', 'interface', 'Audio'); K2 = os.path.join(HERE, 'snd', 'kenney', 'ui', 'Audio')
def kn(n):
    p_ = os.path.join(K1, n + '.ogg'); return p_ if os.path.exists(p_) else os.path.join(K2, n + '.ogg')
def k(n, at=0, db=0, semis=0, **kw): return ev(kn(n), at, db, semis, **kw)

def gen(name, expr, dur):
    """A generated tone (expression of t) as a stereo wav."""
    out = os.path.join(GEN, name + '.wav')
    subprocess.run([FF, '-hide_banner', '-loglevel', 'error', '-y', '-f', 'lavfi', '-i', "aevalsrc='%s|%s':s=48000:d=%.3f" % (expr, expr, dur), out], check=True)
    return out
def ping(name, f, decay=40, att=.004, dur=.4, partial=0.0):
    e = 'sin(2*PI*%g*t)' % f + ('+%g*sin(2*PI*%g*t)' % (partial, f * 2.01) if partial else '')
    return gen(name, '0.5*(%s)*exp(-t*%g)*min(1,t/%g)' % (e, decay, att), dur)
def glide(name, f0, f1, T, decay=3, att=.02):
    return gen(name, '0.4*sin(2*PI*(%g*t+%g*t*t/(2*%g)))*exp(-t*%g)*min(1,t/%g)' % (f0, f1 - f0, T, decay, att), T)
def bloom(name, freqs, att=.12, decay=2.2, dur=1.6):
    e = '+'.join('sin(2*PI*%g*t)' % f for f in freqs)
    return gen(name, '%g*(%s)*exp(-t*%g)*min(1,t/%g)' % (0.6 / len(freqs), e, decay, att), dur)
def noise_swoosh(at, f0, f1, d, db): return ev(gen('n_%d_%d' % (f0, f1), '0', d), at, -120)  # placeholder (unused)
def swoosh(name, d, up=True):
    # filtered noise that rises (or falls) and swells
    out = os.path.join(GEN, name + '.wav')
    f0, f1 = (500, 5000) if up else (5000, 500)
    subprocess.run([FF, '-hide_banner', '-loglevel', 'error', '-y', '-f', 'lavfi', '-i', 'anoisesrc=d=%g:c=pink:a=0.5:r=48000' % d,
                    '-af', "aformat=channel_layouts=stereo,bandpass=f=%d:w=1500,afade=t=in:d=%g,afade=t=out:st=%g:d=%g,volume=0.8" % ((f0 + f1) // 2, d * .6, d * .6, d * .4),
                    out], check=True)
    return out
def g(path, at=0, db=0, **kw): return ev(path, at, db, src_at=0, **kw)

hz = lambda n: 440 * 2 ** ((midi(n) - 69) / 12)
ROLES = [
  ('move', 'Moving between games', -18, [
    ('Air tick', [g(ping('m1', 1800, 90, .002, .12))], .25, .15),
    ('Soft click', [k('click1', 0, -6, hp=1800), g(ping('m2', 2400, 160, .001, .06), 0, -10)], .2, .08),
    ('Glass tick', [g(ping('m3', 3100, 110, .002, .1, .35))], .25, .2),
    ('Haptic', [g(ping('m4', 190, 70, .002, .08)), k('tick_002', 0, -12, hp=2000)], .2, .05),
    ('Pebble', [g(ping('m5', 1250, 60, .003, .15))], .3, .2),
    ('Whisper', [g(ping('m6', 2600, 70, .006, .12), 0, -4), k('click3', 0, -18, hp=3000)], .25, .25),
  ]),
  ('select', 'Choosing a game', -10, [
    ('Two-tone up', [g(ping('s1a', hz('E6'), 28, .004, .25)), g(ping('s1b', hz('B6'), 22, .004, .35), .06)], .6, .25),
    ('Soft confirm', [g(bloom('s2', [hz('C6'), hz('G6')], .006, 9, .45))], .7, .35),
    ('Crisp press', [k('click1', 0, -2), g(ping('s3', 210, 45, .002, .12), 0, -6)], .3, .08),
    ('Bloom', [g(swoosh('s4n', .12), 0, -14), g(ping('s4', 1320, 24, .006, .4), .05)], .7, .35),
    ('Glass pop', [g(glide('s5', 2100, 1600, .09, 30, .002))], .4, .3),
    ('Chime pair', [g(ping('s6a', hz('A5'), 18, .004, .5, .2)), g(ping('s6b', hz('E6'), 16, .004, .6, .2), .09)], .9, .4),
  ]),
  ('back', 'Going back', -12, [
    ('Two-tone down', [g(ping('b1a', hz('B6'), 28, .004, .25)), g(ping('b1b', hz('E6'), 22, .004, .35), .06)], .6, .25),
    ('Soft low', [g(ping('b2', hz('G5'), 26, .006, .35))], .5, .3),
    ('Crisp release', [k('mouserelease1', 0, -2), g(ping('b3', 160, 50, .002, .12), 0, -8)], .3, .08),
    ('Fold', [g(swoosh('b4n', .12, False), 0, -14), g(glide('b4', 1300, 900, .14, 18, .004), .03)], .5, .3),
    ('Glass drop', [g(glide('b5', 1700, 1200, .1, 28, .002))], .4, .3),
    ('Chime down', [g(ping('b6a', hz('E6'), 18, .004, .5, .2)), g(ping('b6b', hz('A5'), 16, .004, .6, .2), .09)], .9, .4),
  ]),
  ('disc', 'Starting a disc game', -8, [
    ('Swoosh and bloom', [g(swoosh('d1n', .7), 0, -10), g(bloom('d1', [hz('D5'), hz('A5'), hz('E6'), hz('F#6')], .25, 1.6, 2.0), .55)], 2.6, .45),
    ('Glide', [g(glide('d2', 300, 900, 1.1, .8, .2), 0, -4), g(bloom('d2b', [hz('A5'), hz('E6')], .05, 2.2, 1.4), 1.05)], 2.6, .4),
    ('Tap and bloom', [k('click1', 0, -4), g(bloom('d3', [hz('C5'), hz('G5'), hz('D6'), hz('E6')], .4, 1.3, 2.2), .15)], 2.6, .5),
    ('Lift', [g(swoosh('d4n', .9), 0, -12), g(ping('d4a', hz('E5'), 6, .02, .9), .5, -4), g(ping('d4b', hz('B5'), 5, .02, 1.2), .9, -4)], 2.6, .45),
    ('Pulse', [g(ping('d5a', 880, 20, .01, .3), t, -6 - i) for i, t in enumerate([0, .35, .62, .82])] + [g(bloom('d5', [hz('A5'), hz('C#6'), hz('E6')], .05, 2, 1.4), 1.0)], 2.6, .4),
    ('Minimal', [k('click1', 0, -6), g(ping('d6', hz('E6'), 5, .02, 1.4, .15), .2, -2)], 2.0, .45),
  ]),
  ('cart', 'Starting a cartridge game', -8, [
    ('Snap, no drum', [k('switch33', 0, -6), k('switch20', .5, -4), k('click2', .51, -10), k('tick_002', .62, -8)], 1.2, .05),
    ('Snap and bloom', [k('switch20', .45, -4), g(bloom('k2', [hz('D5'), hz('A5'), hz('E6')], .08, 2.2, 1.4), .52)], 2.0, .4),
    ('Click and glide', [k('click1', .45, -4), g(glide('k3', 400, 800, .6, 1.5, .05), .5, -4)], 1.4, .35),
    ('Seat and chime', [g(ping('k4', 170, 40, .002, .15), .45), k('switch20', .45, -6), g(ping('k4b', hz('E6'), 8, .01, 1.0, .2), .6, -4)], 1.8, .4),
    ('Minimal', [k('switch20', .45, -4), g(ping('k5', hz('A5'), 9, .01, .8), .55, -6)], 1.5, .35),
  ]),
]
data = []
for key, title, peak, cands in ROLES:
    items = []
    for i, (name, evs, length, wet) in enumerate(cands):
        out = render('sys_%s_%d' % (key, i), evs, length, wet=wet, peak=peak)
        letter = {'move': 'M', 'select': 'S', 'back': 'B', 'disc': 'D', 'cart': 'K'}[key]
        items.append({'id': '%s%d' % (letter, 21 + i), 'name': name, 'src': 'data:audio/ogg;base64,' + base64.b64encode(open(out, 'rb').read()).decode()})
    data.append({'key': key, 'title': title, 'items': items})
json.dump(data, open(os.path.join(HERE, 'picker.json'), 'w'))
print([(g_['key'], len(g_['items'])) for g_ in data], os.path.getsize(os.path.join(HERE, 'picker.json')) // 1000, 'KB')
