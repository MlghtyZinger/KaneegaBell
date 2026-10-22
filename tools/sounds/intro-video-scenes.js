// Original KB video concepts, drawn at explicit times for frame-accurate export.
// These are new scenes, not screen recordings of the app's existing intros.
(() => {
  const canvas = document.querySelector('canvas');
  const ctx = canvas.getContext('2d', { alpha: false });
  const W = 1920, H = 1080, CX = W / 2, CY = H / 2;
  canvas.width = W; canvas.height = H;
  const clamp = (x, a = 0, b = 1) => Math.max(a, Math.min(b, x));
  const ease = x => 1 - Math.pow(1 - clamp(x), 3);
  const smooth = x => { const p = clamp(x); return p * p * (3 - 2 * p); };
  const logo = new Image();
  window.prepareScene = async svg => {
    logo.src = 'data:image/svg+xml;base64,' + btoa(unescape(encodeURIComponent(svg)));
    await logo.decode();
  };
  function background(inner, outer, x = CX, y = CY) {
    const g = ctx.createRadialGradient(x, y, 20, CX, CY, 1100);
    g.addColorStop(0, inner); g.addColorStop(1, outer);
    ctx.fillStyle = g; ctx.fillRect(0, 0, W, H);
  }
  function wordmark(t, start, color, y = 735, spacing = 12, size = 40) {
    const p = ease((t - start) / .75);
    if (p <= 0) return;
    ctx.save(); ctx.globalAlpha = p;
    ctx.fillStyle = color; ctx.font = `500 ${size}px Bahnschrift, 'Segoe UI', sans-serif`;
    const text = 'KANEEGABELL';
    const widths = [...text].map(letter => ctx.measureText(letter).width);
    const total = widths.reduce((a, b) => a + b, 0) + spacing * (text.length - 1);
    let x = CX - total / 2;
    for (let i = 0; i < text.length; i++) { ctx.fillText(text[i], x, y + 16 * (1 - p)); x += widths[i] + spacing; }
    ctx.restore();
  }
  function emblem(x, y, size, opacity = 1) {
    ctx.save(); ctx.globalAlpha = opacity;
    ctx.drawImage(logo, x - size / 2, y - size / 2, size, size); ctx.restore();
  }
  function line(points, color, width = 2, opacity = 1) {
    ctx.save(); ctx.strokeStyle = color; ctx.lineWidth = width; ctx.globalAlpha = opacity;
    ctx.lineCap = 'round'; ctx.lineJoin = 'round'; ctx.beginPath();
    points.forEach(([x, y], i) => i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)); ctx.stroke(); ctx.restore();
  }
  function ring(x, y, radius, color, opacity, width = 2) {
    if (opacity <= 0 || radius <= 0) return;
    ctx.save(); ctx.globalAlpha = opacity; ctx.strokeStyle = color; ctx.lineWidth = width;
    ctx.beginPath(); ctx.arc(x, y, radius, 0, Math.PI * 2); ctx.stroke(); ctx.restore();
  }
  function prism(t, energy) {
    background('#ffffff', '#e4e7f2', CX, 350);
    const colors = ['#6c4ad3', '#4858d4', '#447fda', '#7799e3', '#a5addc', '#6e4aaf'];
    // Six sheets of glass arrive on the opening notes and close into one mark.
    const cues = [.54, .64, .75, .87, 1.03, 1.21];
    const settle = ease((t - 1.22) / .8);
    for (let i = 0; i < 6; i++) {
      const p = ease((t - cues[i]) / .42);
      if (!p) continue;
      const angle = i * Math.PI / 3 + (1 - settle) * .8;
      const radius = 125 + (1 - p) * 570 - settle * 75;
      const x = CX + Math.cos(angle) * radius, y = 458 + Math.sin(angle) * radius * .7;
      ctx.save(); ctx.translate(x, y); ctx.rotate(angle + .4 * (1 - p));
      ctx.globalAlpha = (.42 + .2 * energy) * p * (1 - .65 * settle);
      const g = ctx.createLinearGradient(-150, -90, 150, 90); g.addColorStop(0, colors[i]); g.addColorStop(1, '#ffffff');
      ctx.fillStyle = g; ctx.beginPath(); ctx.moveTo(-145, -95); ctx.lineTo(150, -52); ctx.lineTo(105, 95); ctx.lineTo(-110, 65); ctx.closePath(); ctx.fill();
      ctx.globalAlpha = .35 * p; ctx.strokeStyle = '#ffffff'; ctx.lineWidth = 2; ctx.stroke(); ctx.restore();
    }
    // The physical mark resolves from the sheets; the last chime paints its orbit.
    const p = ease((t - 1.2) / .85);
    emblem(CX, 460 + (1 - p) * 35, 260 * (.85 + .15 * p), p);
    const chime = clamp((t - 2.98) / .65);
    if (chime > 0) {
      const a = -Math.PI / 2 + chime * Math.PI * 2;
      ctx.save(); ctx.strokeStyle = '#7278c1'; ctx.lineWidth = 1.5; ctx.globalAlpha = .45 * (1 - .6 * chime);
      ctx.beginPath(); ctx.ellipse(CX, 460, 225, 100, -.25, -Math.PI / 2, a); ctx.stroke(); ctx.restore();
      const x = CX + 225 * Math.cos(a), y = 460 + 100 * Math.sin(a);
      const flare = Math.sin(chime * Math.PI);
      const g = ctx.createRadialGradient(x, y, 0, x, y, 60); g.addColorStop(0, `rgba(255,255,255,${flare})`); g.addColorStop(1, 'rgba(255,255,255,0)'); ctx.fillStyle = g; ctx.fillRect(x - 60, y - 60, 120, 120);
    }
    wordmark(t, 1.65, '#353554', 715, 10, 42);
    const shine = clamp((t - 2.95) / .7);
    if (shine > 0 && shine < 1) {
      const x = 660 + shine * 650;
      const g = ctx.createLinearGradient(x - 90, 0, x + 90, 0); g.addColorStop(0, 'rgba(255,255,255,0)'); g.addColorStop(.5, 'rgba(255,255,255,.68)'); g.addColorStop(1, 'rgba(255,255,255,0)');
      ctx.fillStyle = g; ctx.fillRect(620, 720, 680, 2);
    }
  }
  function project([x, y, z], yaw, pitch) {
    const a = x * Math.cos(yaw) + z * Math.sin(yaw);
    const b = -x * Math.sin(yaw) + z * Math.cos(yaw);
    return [a, y * Math.cos(pitch) - b * Math.sin(pitch), y * Math.sin(pitch) + b * Math.cos(pitch)];
  }
  function cube(x, y, size, turn, opacity, color) {
    const vertices = [[-1,-1,-1],[1,-1,-1],[1,1,-1],[-1,1,-1],[-1,-1,1],[1,-1,1],[1,1,1],[-1,1,1]].map(v => project(v.map(n => n * size / 2), turn, -.5));
    const faces = [[0,1,2,3],[4,7,6,5],[0,4,5,1],[3,2,6,7],[1,5,6,2],[0,3,7,4]].map((ids, i) => ({ ids, i, depth: ids.reduce((s, n) => s + vertices[n][2], 0) / 4 })).sort((a, b) => a.depth - b.depth);
    ctx.save(); ctx.translate(x, y);
    for (const { ids, i } of faces) {
      ctx.beginPath(); ids.forEach((id, n) => n ? ctx.lineTo(vertices[id][0], vertices[id][1]) : ctx.moveTo(vertices[id][0], vertices[id][1])); ctx.closePath();
      ctx.globalAlpha = opacity * (.2 + i * .08); ctx.fillStyle = color; ctx.fill(); ctx.globalAlpha = opacity * .65; ctx.lineWidth = 1.2; ctx.strokeStyle = '#cfc5ff'; ctx.stroke();
    }
    ctx.restore();
  }
  function orbit(t, energy) {
    background('#15172d', '#030409', CX, 420);
    // A restrained star field gives the scene depth without a moving wallpaper.
    for (let i = 0; i < 55; i++) {
      const x = (i * 541 + 113) % W, y = (i * 277 + 97) % H;
      ctx.fillStyle = `rgba(157,153,214,${.06 + .06 * Math.sin(i + t * .5)})`; ctx.fillRect(x, y, 2, 2);
    }
    const start = 1.19, end = 5.23;
    const p = clamp((t - start) / (end - start));
    const yaw = -.8 + .2 * Math.sin(t * .18);
    const count = 8;
    for (let i = 0; i < count; i++) {
      const cue = [1.19,1.63,2.12,2.42,2.9,3.45,3.97,4.76][i];
      const arrived = ease((t - cue) / .35);
      if (!arrived) continue;
      const a = i / count * Math.PI * 2 - Math.PI / 2;
      const close = ease((t - 4.76) / .48);
      const radius = 245 * (1 - close) + 93 * close;
      const world = project([Math.cos(a) * radius, Math.sin(a) * radius, 0], yaw, -.3);
      const x = CX + world[0], y = 452 + world[1] - 350 * (1 - arrived);
      const opacity = arrived * (1 - ease((t - 5.23) / .45));
      cube(x, y, 62 + energy * 5, t * .25 + i * .8, opacity, '#8976ef');
      const pulse = Math.exp(-Math.max(0, t - cue) * 8);
      ring(x, y, 36 + (t - cue) * 50, '#8b7aef', pulse * .35);
    }
    // Elliptical construction lines draw with the chimes, then collapse cleanly.
    ctx.save(); ctx.translate(CX, 452); ctx.scale(1, .45); ctx.rotate(.2);
    ctx.strokeStyle = '#8c7bd9'; ctx.lineWidth = 1.5; ctx.globalAlpha = .25 * smooth(p * 3) * (1 - ease((t - 5.23) / .65));
    ctx.beginPath(); ctx.arc(0, 0, 300, -Math.PI / 2, -Math.PI / 2 + Math.PI * 2 * p); ctx.stroke(); ctx.restore();
    const land = ease((t - 5.23) / .6);
    if (land > 0) {
      const g = ctx.createRadialGradient(CX, 452, 10, CX, 452, 350); g.addColorStop(0, `rgba(114,91,206,${.28 * land})`); g.addColorStop(1, 'rgba(114,91,206,0)'); ctx.fillStyle = g; ctx.fillRect(CX - 350, 102, 700, 700);
      emblem(CX, 452 + 45 * (1 - land), 265 * (.76 + .24 * land), land);
      const pulse = Math.exp(-(t - 5.23) * 2.1); ring(CX, 452, 150 + (t - 5.23) * 140, '#bfb1ff', pulse * .8, 1.2);
    }
    wordmark(t, 5.34, '#ddd8ef', 722, 13, 38);
  }
  function aurora(t, energy) {
    background('#112944', '#02050c', CX, 500);
    const appear = ease((t - .72) / 1.4);
    // Layered luminous ribbons curve in 3D-like perspective and converge on the swell.
    const resolve = ease((t - 3.8) / .7);
    ctx.save(); ctx.globalCompositeOperation = 'screen';
    for (let band = 0; band < 14; band++) {
      const points = [];
      for (let step = 0; step <= 120; step++) {
        const u = step / 120, x = u * W;
        const envelope = Math.sin(u * Math.PI);
        const height = Math.sin(u * 6.2 + t * .52 + band * .09) * (130 + band * 5);
        const y = 500 + height * envelope + (band - 7) * 9 + Math.sin(u * 3.1 - t * .3) * 55;
        points.push([x, y]);
      }
      line(points, band % 3 === 0 ? '#9eeaff' : '#5b9bc7', band % 3 === 0 ? 2 : 1, appear * (.05 + .08 * energy) * (1 - .45 * resolve));
      if (band === 7) { ctx.shadowColor = '#6ad7ff'; ctx.shadowBlur = 18; line(points, '#b5f2ff', 2, appear * .36); ctx.shadowBlur = 0; }
    }
    for (let i = 0; i < 64; i++) {
      const speed = .2 + (i % 7) * .035;
      const x = (i * 419 + t * 14 * speed) % W;
      const y = 330 + ((i * 173) % 370) + Math.sin(t * .5 + i) * 35;
      const alpha = appear * (.1 + .17 * Math.sin(i * 3.1 + t) ** 2) * (1 - .5 * resolve);
      ctx.fillStyle = `rgba(181,224,255,${alpha})`; ctx.beginPath(); ctx.arc(x, y, i % 5 === 0 ? 2 : 1, 0, Math.PI * 2); ctx.fill();
    }
    ctx.restore();
    const swell = Math.exp(-Math.pow((t - 4.28) / .42, 2));
    const g = ctx.createRadialGradient(CX, 450, 5, CX, 450, 580);
    g.addColorStop(0, `rgba(149,210,241,${swell * .44})`); g.addColorStop(.4, `rgba(55,133,194,${swell * .18})`); g.addColorStop(1, 'rgba(20,75,125,0)'); ctx.fillStyle = g; ctx.fillRect(380, 0, 1160, H);
    const logoIn = ease((t - 3.96) / .68);
    if (logoIn) {
      ctx.save(); ctx.shadowColor = '#8ddcfc'; ctx.shadowBlur = 32 * (1 - logoIn);
      emblem(CX, 448, 250 * (1.08 - .08 * logoIn), logoIn); ctx.restore();
    }
    wordmark(t, 4.12, '#e7f1f8', 705, 15, 36);
    const rule = ease((t - 4.28) / .8);
    line([[CX - 75 * rule, 755],[CX + 75 * rule, 755]], '#96c5e1', 1, .5 * rule);
  }
  window.drawScene = (name, time, energy, duration) => {
    ctx.globalAlpha = 1; ctx.globalCompositeOperation = 'source-over'; ctx.shadowBlur = 0;
    if (name === 'prism') prism(time, energy);
    else if (name === 'orbit') orbit(time, energy);
    else if (name === 'aurora') aurora(time, energy);
    else throw new Error('Unknown scene: ' + name);
    // A short, baked final fade keeps playback transitions independent of CPU timing.
    const fade = smooth((time - (duration - .28)) / .28);
    if (fade > 0) { ctx.fillStyle = `rgba(0,0,0,${fade})`; ctx.fillRect(0, 0, W, H); }
  };
})();
