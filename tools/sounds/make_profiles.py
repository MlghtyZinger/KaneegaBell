"""Three complete sound profiles (Tactile, Console, Retro console), each with move/select/back and a whole disc and
cartridge game start. Original sounds: generated tones plus Kenney's CC0 click recordings. Writes profiles.js."""
import base64, json, os, subprocess
HERE = os.path.dirname(os.path.abspath(__file__))
ms = open(os.path.join(HERE, 'make_system.py'), encoding='utf8').read()
exec(ms[:ms.index('hz = lambda n:')])          # engine + gen/ping/glide/bloom/swoosh/k/g helpers
hz = lambda n: 440 * 2 ** ((midi(n) - 69) / 12)
def square(name, f, decay=60, att=.002, dur=.15):
    # a square wave, softened later with a low-pass so it sounds warm, not chiptune-harsh
    return gen(name, '0.25*(2*gt(sin(2*PI*%g*t),0)-1)*exp(-t*%g)*min(1,t/%g)' % (f, decay, att), dur)
def sqglide(name, f0, f1, T, decay=2):
    return gen(name, '0.22*(2*gt(sin(2*PI*(%g*t+%g*t*t/(2*%g))),0)-1)*exp(-t*%g)*min(1,t/%g)' % (f0, f1 - f0, T, decay, .01), T)

P = {
  'tactile': {   # real buttons, latches and a small haptic thump: no tones at all
    'move': ([k('click3', 0, -4, hp=1500), g(ping('t_m', 150, 90, .001, .05), 0, -10)], .2, .04, -18),
    'select': ([k('mouseclick1', 0, 0), k('mouserelease1', .07, -4)], .3, .04, -10),
    'back': ([k('switch2', 0, 0, -3)], .3, .04, -12),
    'disc': ([k('switch14', .02, -4), k('scratch_002', .45, -12, -6), k('switch29', .95, -4), k('click2', 1.15, -6)], 1.6, .05, -8),
    'cart': ([k('switch33', 0, -6), k('switch20', .5, -4), k('click2', .51, -10), k('tick_002', .62, -8)], 1.2, .05, -8),
  },
  'console': {   # clean, modern system-menu tones with a soft reverb
    'move': ([g(ping('c_m', 3100, 110, .002, .1, .35))], .25, .2, -18),
    'select': ([g(ping('c_s1', hz('E6'), 28, .004, .25)), g(ping('c_s2', hz('B6'), 22, .004, .35), .06)], .6, .25, -10),
    'back': ([g(ping('c_b1', hz('B6'), 28, .004, .25)), g(ping('c_b2', hz('E6'), 22, .004, .35), .06)], .6, .25, -12),
    'disc': ([k('click1', 0, -8), g(swoosh('c_dn', .7), .1, -10), g(bloom('c_d', [hz('D5'), hz('A5'), hz('E6'), hz('F#6')], .25, 1.6, 2.0), .62)], 2.7, .45, -8),
    'cart': ([k('switch20', .5, -4), g(bloom('c_k', [hz('D5'), hz('A5'), hz('E6')], .08, 2.2, 1.4), .56)], 2.0, .4, -8),
  },
  'retro': {     # CRT-era hardware: warm filtered square beeps, chunky switches, a power thunk
    'move': ([g(square('r_m', 1000, 90, .002, .06), 0, 0, lp=2400)], .15, .08, -18),
    'select': ([k('switch33', 0, -12, -2), g(square('r_s1', hz('A5'), 40, .002, .09), .01, 0, lp=2600), g(square('r_s2', hz('E6'), 30, .002, .14), .08, 0, lp=2600)], .4, .12, -10),
    'back': ([g(square('r_b1', hz('E6'), 40, .002, .09), 0, 0, lp=2600), g(square('r_b2', hz('A5'), 30, .002, .14), .07, 0, lp=2600)], .35, .12, -12),
    'disc': ([k('switch33', 0, -4, -3), g(ping('r_dt', 60, 12, .002, .4), .02, -2), g(sqglide('r_dg', 200, 600, .55), .35, -6, lp=1800),
              g(square('r_d1', hz('A5'), 8, .004, .5), 1.0, -4, lp=2400), g(square('r_d2', hz('E6'), 5, .004, .9), 1.12, -4, lp=2400)], 2.1, .2, -8),
    'cart': ([k('switch33', .45, -2, -3), g(ping('r_kt', 70, 14, .002, .35), .47, -2), g(square('r_k1', hz('E6'), 14, .003, .3), .6, -4, lp=2400),
              g(square('r_k2', hz('A6'), 8, .003, .6), .7, -4, lp=2400)], 1.5, .2, -8),
  },
}
out = {}
for prof, roles in P.items():
    for role, (evs, length, wet, peak) in roles.items():
        f = render('prof_%s_%s' % (prof, role), evs, length, wet=wet, peak=peak)
        out['prof_%s_%s' % (prof, role)] = 'data:audio/ogg;base64,' + base64.b64encode(open(f, 'rb').read()).decode()
open(os.path.join(HERE, 'profiles.js'), 'w').write('Object.assign(SOUNDS, ' + json.dumps(out) + ');\n')
print(len(out), 'sounds', os.path.getsize(os.path.join(HERE, 'profiles.js')) // 1000, 'KB')
