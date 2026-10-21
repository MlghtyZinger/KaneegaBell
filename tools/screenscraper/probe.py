"""A throwaway test of ScreenScraper as a metadata source (not part of KB): looks up some games of a library on its
jeuInfos API and reports how many it recognises, which pictures each has (front, back, spine, disc/cartridge,
manual...) and how that compares with what LaunchBox gave the same games.

Needs ScreenScraper's developer credentials (requested from its forum by KB's developer) and a member login, from
the environment (never from files or the command line, and never printed):

    $env:SS_DEVID, $env:SS_DEVPASSWORD, $env:SS_USER, $env:SS_PASSWORD

    python tools/screenscraper/probe.py --db <library.db> [--per-platform 3] [--all] [--dry]

--dry lists what would be asked without calling the API (no credentials needed). Results go to
tools/screenscraper/probe-results.json (git-ignored). Requests are sequential, 1.5 s apart, and stop on the first
rate-limit or quota error (429 / 430 / 431)."""
import argparse, hashlib, json, os, sqlite3, sys, time, urllib.error, urllib.parse, urllib.request, zlib

API = 'https://api.screenscraper.fr/api2/'
SOFTNAME = 'KaneegaBell-probe'
# ScreenScraper system ids for KB's platforms (checked against systemesListe.php when run with credentials).
SYSTEMS = {'snes': 4, 'n64': 14, 'gba': 12, 'nds': 15, '3ds': 17, 'gc': 13, 'wii': 16, 'ps1': 57, 'ps2': 58, 'ps3': 59,
           'psp': 61, 'saturn': 22, 'segacd': 20, 'switch': 225, 'xbox': 32, 'xbox360': 33}
CART = {'snes', 'n64', 'gba', 'nds'}   # small enough to hash whole
MAX_HASH = 128 * 1024 * 1024
WANT = ['box-2D', 'box-2D-back', 'box-2D-side', 'box-3D', 'support-2D', 'manuel', 'wheel', 'fanart', 'ss', 'sstitle']


def hashes(path):
    crc, md5, sha1 = 0, hashlib.md5(), hashlib.sha1()
    with open(path, 'rb') as f:
        while chunk := f.read(1 << 20):
            crc = zlib.crc32(chunk, crc); md5.update(chunk); sha1.update(chunk)
    return '%08X' % crc, md5.hexdigest().upper(), sha1.hexdigest().upper()


def call(endpoint, params, creds):
    q = dict(params, devid=creds['devid'], devpassword=creds['devpassword'], softname=SOFTNAME, output='json',
             ssid=creds['user'], sspassword=creds['password'])
    req = urllib.request.Request(API + endpoint + '?' + urllib.parse.urlencode(q), headers={'User-Agent': SOFTNAME})
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return r.status, json.loads(r.read().decode('utf-8', 'replace') or 'null')
    except urllib.error.HTTPError as e:   # the URL (with the login in it) is never printed
        return e.code, None
    except (urllib.error.URLError, ValueError) as e:
        return 0, str(type(e).__name__)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--db', required=True)
    ap.add_argument('--per-platform', type=int, default=3)
    ap.add_argument('--all', action='store_true')
    ap.add_argument('--dry', action='store_true')
    a = ap.parse_args()
    db = sqlite3.connect('file:%s?mode=ro' % a.db.replace('\\', '/'), uri=True)
    rows = db.execute("""SELECT g.id, g.platform_id, g.title, g.launch_path, g.lb_id,
        EXISTS(SELECT 1 FROM lb_images i WHERE i.db_id = g.lb_id AND i.kind IN ('Box - Back', 'Box - Back - Reconstructed')),
        EXISTS(SELECT 1 FROM lb_images i WHERE i.db_id = g.lb_id AND i.kind = 'Box - Spine'),
        EXISTS(SELECT 1 FROM lb_images i WHERE i.db_id = g.lb_id AND i.kind = 'Disc')
        FROM games g WHERE g.missing = 0 ORDER BY g.platform_id, g.id""").fetchall()
    picked, per = [], {}
    for r in rows:
        if r[1] not in SYSTEMS:
            continue
        if not a.all and per.get(r[1], 0) >= a.per_platform:
            continue
        per[r[1]] = per.get(r[1], 0) + 1
        picked.append(r)
    print('%d games from %d platforms' % (len(picked), len(per)))
    creds = {'devid': os.environ.get('SS_DEVID'), 'devpassword': os.environ.get('SS_DEVPASSWORD'),
             'user': os.environ.get('SS_USER'), 'password': os.environ.get('SS_PASSWORD')}
    if not a.dry and not all(creds.values()):
        sys.exit('Set SS_DEVID, SS_DEVPASSWORD, SS_USER and SS_PASSWORD first (see the top of this file), or use --dry.')
    out, matched, media_count, lb_has, ss_gain = [], 0, {}, {'back': 0, 'spine': 0, 'disc': 0}, {'back': 0, 'spine': 0, 'disc': 0}
    t0 = time.time()
    for id_, plat, title, path, lb_id, lb_back, lb_spine, lb_disc in picked:
        name = os.path.basename(path)
        size = os.path.getsize(path) if os.path.exists(path) else None
        p = {'systemeid': SYSTEMS[plat], 'romnom': name}
        if size is not None:
            p['romtaille'] = size
        h = None
        if plat in CART and size is not None and size <= MAX_HASH and not a.dry:
            h = hashes(path)
            p['crc'], p['md5'], p['sha1'] = h
        if a.dry:
            print('%-8s %-50s %s' % (plat, title[:50], 'hash' if plat in CART and size else 'name+size' if size else 'name only (file not found)'))
            continue
        status, data = call('jeuInfos.php', p, creds)
        time.sleep(1.5)
        if status in (429, 430, 431):
            print('stopped: ScreenScraper answered %d (rate limit / daily quota / too many unrecognised ROMs)' % status)
            break
        if status == 401 or status == 403:
            sys.exit('ScreenScraper refused the login or the developer credentials (%d).' % status)
        game = (data or {}).get('response', {}).get('jeu') if isinstance(data, dict) else None
        types = {}
        if game:
            matched += 1
            for m in game.get('medias', []):
                types.setdefault(m.get('type'), []).append(m.get('region'))
            for t in types:
                media_count[t] = media_count.get(t, 0) + 1
            got = {'back': 'box-2D-back' in types, 'spine': 'box-2D-side' in types, 'disc': 'support-2D' in types}
            for k, lb in (('back', lb_back), ('spine', lb_spine), ('disc', lb_disc)):
                lb_has[k] += bool(lb)
                ss_gain[k] += bool(got[k] and not lb)
        out.append({'id': id_, 'platform': plat, 'title': title, 'status': status, 'matched': bool(game),
                    'by_hash': bool(h), 'media': {t: sorted(set(x for x in r if x)) for t, r in types.items()},
                    'launchbox': {'back': bool(lb_back), 'spine': bool(lb_spine), 'disc': bool(lb_disc)}})
        print('%-8s %-45s %s  %s' % (plat, title[:45], 'OK ' if game else '-- (%s)' % status,
                                     ' '.join(t for t in WANT if t in types)))
    if a.dry:
        return
    n = len(out)
    print('\nAsked about %d games in %.0f s: %d recognised.' % (n, time.time() - t0, matched))
    print('Pictures ScreenScraper has (games):', {t: media_count.get(t, 0) for t in WANT})
    print('LaunchBox already has (of the same games):', lb_has)
    print('ScreenScraper adds where LaunchBox has none:', ss_gain)
    json.dump(out, open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'probe-results.json'), 'w'), indent=1)


main()
