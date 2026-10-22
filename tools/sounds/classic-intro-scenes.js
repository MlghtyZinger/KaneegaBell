// Console-style motion with KB branding. Optional PS3 typeface is loaded by the renderer.
(() => {
  const canvas = document.querySelector('canvas');
  const ctx = canvas.getContext('2d', { alpha: false });
  canvas.width = 1920; canvas.height = 1080;
  const W = 1920, H = 1080, CX = 960;
  const clamp = x => Math.max(0, Math.min(1, x));
  const ease = x => 1 - Math.pow(1 - clamp(x), 3);
  const smooth = x => { const p = clamp(x); return p * p * (3 - 2 * p); };
  const marks = new Map();
  let brandImage;
  window.prepareScene = async svg => {
    // Keep the distinctive bull silhouette, remove the rounded app-icon tile.
    const bare = svg.replace(/<rect width="256" height="256"[^>]*\/>/, '');
    const image = new Image();
    image.src = 'data:image/svg+xml;base64,' + btoa(unescape(encodeURIComponent(bare)));
    await image.decode();
    brandImage = image;
    for (const color of ['#343383', '#a99ce8', '#f5f5f5', '#d6241d', '#081e4a', '#73b900', '#e79c17', '#252525', '#e62b08', '#8e1705', '#e8c51e', '#39a99d', '#526fad', '#315d12']) {
      const stamp = document.createElement('canvas'); stamp.width = 512; stamp.height = 512;
      const c = stamp.getContext('2d'); c.drawImage(image, 0, 0, 512, 512);
      c.globalCompositeOperation = 'source-in'; c.fillStyle = color; c.fillRect(0, 0, 512, 512);
      // Retain the original eye shapes, nostrils and nose-ring opening as cutouts.
      c.globalCompositeOperation = 'destination-out'; c.scale(2, 2);
      c.fill(new Path2D('M92 126 C100 119 112 121 119 131 C110 133 100 133 92 126 Z M164 126 C156 119 144 121 137 131 C146 133 156 133 164 126 Z'));
      for (const x of [114, 142]) { c.beginPath(); c.ellipse(x, 196, 5, 7.5, 0, 0, Math.PI * 2); c.fill(); }
      c.beginPath(); c.arc(128, 214, 11, 0, Math.PI * 2); c.fill();
      marks.set(color, stamp);
    }
  };
  function mark(x, y, size, color, alpha = 1) {
    ctx.save(); ctx.globalAlpha *= alpha;
    ctx.drawImage(marks.get(color), x - size / 2, y - size / 2, size, size); ctx.restore();
  }
  function tracking(text, x, y, font, spacing, color, alpha = 1) {
    ctx.save(); ctx.font = font; ctx.fillStyle = color; ctx.globalAlpha *= alpha;
    const widths = [...text].map(c => ctx.measureText(c).width);
    let left = x - (widths.reduce((a, b) => a + b, 0) + spacing * (text.length - 1)) / 2;
    [...text].forEach((c, i) => { ctx.fillText(c, left, y); left += widths[i] + spacing; });
    ctx.restore();
  }
  function gba(t) {
    ctx.fillStyle = '#ffffff'; ctx.fillRect(0, 0, W, H);
    const text = 'Kaneegabell';
    const font = 'italic 900 128px "Arial Black", Arial, sans-serif';
    ctx.font = font;
    const widths = [...text].map(c => ctx.measureText(c).width);
    const total = widths.reduce((a, b) => a + b, 0) - 3 * (text.length - 1);
    let x = CX - total / 2;
    for (let i = 0; i < text.length; i++) {
      const p = clamp((t - (.25 + i * .064)) / .31);
      if (p > 0) {
        const flight = 1 - ease(p);
        const y = 492 + flight * (285 + i * 12) - Math.sin(p * Math.PI) * 52;
        const xx = x + flight * (400 + i * 22);
        const settle = smooth((p - .62) / .38);
        for (let echo = 4; echo >= 0; echo--) {
          ctx.save(); ctx.font = font; ctx.textBaseline = 'alphabetic';
          ctx.translate(xx + echo * 15 * flight, y + echo * 18 * flight);
          ctx.rotate(flight * .36); ctx.scale(1, 1 - flight * .14);
          ctx.globalAlpha = echo ? (1 - settle) * .38 : 1;
          ctx.fillStyle = echo ? `hsl(${(i * 42 + echo * 65) % 360} 85% 56%)` : settle > .93 ? '#343383' : `hsl(${(i * 43 + p * 210) % 360} 75% ${40 + 8 * (1 - settle)}%)`;
          ctx.fillText(text[i], 0, 0); ctx.restore();
        }
      }
      x += widths[i] - 3;
    }
    mark(CX, 650, 105, '#343383', ease((t - 1.3) / .18));
    // The last chime starts a white glint across the actual wordmark and bull.
    if (t >= 2.98 && t < 3.5) {
      const p = (t - 2.98) / .52;
      const shine = document.createElement('canvas'); shine.width = W; shine.height = H;
      const c = shine.getContext('2d');
      c.font = font; c.fillStyle = '#fff';
      let left = CX - total / 2;
      for (let i = 0; i < text.length; i++) { c.fillText(text[i], left, 492); left += widths[i] - 3; }
      c.drawImage(marks.get('#343383'), CX - 52.5, 597.5, 105, 105);
      c.globalCompositeOperation = 'source-in';
      const sx = CX - total / 2 - 120 + p * (total + 240);
      const stripe = c.createLinearGradient(sx - 90, 0, sx + 90, 0);
      stripe.addColorStop(0, 'rgba(255,255,255,0)'); stripe.addColorStop(.5, '#fff'); stripe.addColorStop(1, 'rgba(255,255,255,0)');
      c.fillStyle = stripe; c.fillRect(0, 0, W, H); ctx.drawImage(shine, 0, 0);
    }
  }
  // A single path winds across the top, front and right of a real cube.
  // Each square occupies a cube face, rather than tracing a flat hexagon.
  const L = 285, CELL = L / 3, HALF = L / 2;
  const tiles = [
    ['top',1,0],['top',0,0],['top',0,1],['top',0,2],
    ['front',0,0],['front',0,1],['front',0,2],['front',1,2],['front',2,2],
    // Continue along the bottom, up the OUTER right edge, then turn inward.
    // Reversing this face puts the upright at the center and loses the G.
    ['right',2,2],['right',1,2],['right',0,2],['right',0,1],['right',0,0],['right',1,0],['right',2,0],
  ];
  const beats = [1.19,1.40,1.63,1.88,2.12,2.42,2.66,2.90,3.18,3.45,3.71,3.97,4.17,4.36,4.56,4.76,5.23];
  function facePoint(face, u, v, lift = 0) {
    if (face === 'top') return [-HALF + u, -HALF - lift, -HALF + v];
    if (face === 'front') return [-HALF + u, -HALF + v, HALF + lift];
    return [HALF + lift, -HALF + v, -HALF + u];
  }
  function project([x,y,z]) {
    return [CX + (x - z) * Math.SQRT1_2, 403 + y * .81649658 + (x + z) * .40824829,
      (x + z) * .57735027 - y * .57735027];
  }
  function solid(center, size, angle) {
    const vertices = [[-1,-1,-1],[1,-1,-1],[1,1,-1],[-1,1,-1],[-1,-1,1],[1,-1,1],[1,1,1],[-1,1,1]].map(([x,y,z]) => {
      const xx = x * Math.cos(angle) - y * Math.sin(angle);
      const yy = x * Math.sin(angle) + y * Math.cos(angle);
      const zz = z * Math.cos(angle * .53) - xx * Math.sin(angle * .53);
      const xxx = z * Math.sin(angle * .53) + xx * Math.cos(angle * .53);
      return project([center[0] + xxx * size / 2, center[1] + yy * size / 2, center[2] + zz * size / 2]);
    });
    return [[0,1,2,3],[4,7,6,5],[0,4,5,1],[3,2,6,7],[1,5,6,2],[0,3,7,4]].map((ids,i) => ({
      points: ids.map(id => vertices[id]), color: ['#514184','#9b88da','#d0bdfb','#66539e','#7361ad','#7764b0'][i],
    }));
  }
  function gamecube(t) {
    ctx.fillStyle = '#000'; ctx.fillRect(0,0,W,H);
    if (t < beats[0]) return;
    const segment = Math.min(tiles.length - 1, beats.findLastIndex(beat => t >= beat));
    const fraction = clamp((t - beats[segment]) / (beats[segment + 1] - beats[segment]));
    const landing = ease((t - 5.0) / .23);
    const faces = [];
    for (let i = 0; i <= segment; i++) {
      const [face,col,row] = tiles[i];
      const reveal = i < segment ? 1 : ease(fraction * 3);
      const inset = (1 - reveal) * CELL / 2;
      const u = col * CELL + inset, v = row * CELL + inset, side = CELL - 2 * inset;
      faces.push({points: [[u,v],[u+side,v],[u+side,v+side],[u,v+side]].map(([a,b]) => project(facePoint(face,a,b))),
        color: face === 'top' ? '#b6a2ef' : face === 'front' ? '#8c73ca' : '#65519d'});
    }
    const center = index => {
      const [face,col,row] = tiles[index]; return facePoint(face,(col+.5)*CELL,(row+.5)*CELL,CELL/2);
    };
    const from = center(segment), to = segment < tiles.length-1 ? center(segment+1) : [0,0,0];
    const p = segment === tiles.length-1 ? ease(fraction) : smooth(fraction);
    const position = from.map((v,i) => v + (to[i]-v)*p);
    if (segment < tiles.length-1) position[1] -= Math.sin(fraction*Math.PI)*22;
    // Two diminishing rebounds after the center cube lands on the final chime.
    const rebound = t - 5.23;
    if (rebound >= 0 && rebound < .52) {
      const first = rebound < .32;
      const u = first ? rebound / .32 : (rebound - .32) / .20;
      position[1] -= (first ? 54 : 18) * 4 * u * (1 - u);
    }
    const spin = segment === tiles.length-1 ? (1-p)*Math.PI*2 : fraction*Math.PI/2;
    faces.push(...solid(position,CELL*(.84+.16*landing),spin));
    faces.sort((a,b) => a.points.reduce((s,p)=>s+p[2],0)/a.points.length - b.points.reduce((s,p)=>s+p[2],0)/b.points.length);
    for (const {points,color} of faces) {
      ctx.beginPath(); points.forEach(([x,y],i)=>i?ctx.lineTo(x,y):ctx.moveTo(x,y)); ctx.closePath();
      ctx.fillStyle=color; ctx.fill(); ctx.strokeStyle=color; ctx.lineWidth=.65; ctx.stroke();
    }
    const caption = ease((t - 5.23) / .3);
    tracking('KANEEGABELL',CX,730,'700 52px Bahnschrift, Arial, sans-serif',3,'#f0eef9',caption);
    mark(CX,802,70,'#a99ce8',caption);
  }
  function ps3(t,energy){
    // The later PS3 boot's restrained, warm XMB surface and right-side logo layout.
    const fade=ease((t-.1)/1.25);
    ctx.fillStyle='#000';ctx.fillRect(0,0,W,H);
    const g=ctx.createLinearGradient(0,0,0,H);g.addColorStop(0,'#161312');g.addColorStop(.58,'#302722');g.addColorStop(1,'#181310');
    ctx.save();ctx.globalAlpha=fade;ctx.fillStyle=g;ctx.fillRect(0,0,W,H);
    const glow=ctx.createRadialGradient(1100,580,20,1100,580,1050);glow.addColorStop(0,'rgba(120,99,75,.18)');glow.addColorStop(1,'rgba(25,20,17,0)');ctx.fillStyle=glow;ctx.fillRect(0,0,W,H);
    const points=[];
    for(let i=0;i<=160;i++){
      const u=i/160;
      const y=548+Math.sin(u*5.3+t*.34)*62+Math.sin(u*3.1-t*.22)*34;
      points.push([u*W,y]);
    }
    // A single broad translucent ribbon, rather than the earlier concept's many lines.
    ctx.beginPath();points.forEach(([x,y],i)=>i?ctx.lineTo(x,y):ctx.moveTo(x,y));
    for(let i=points.length-1;i>=0;i--)ctx.lineTo(points[i][0],points[i][1]+88+Math.sin(i/160*Math.PI)*45);
    ctx.closePath();const ribbon=ctx.createLinearGradient(0,460,0,740);ribbon.addColorStop(0,'rgba(215,209,184,.13)');ribbon.addColorStop(.35,'rgba(204,197,176,.08)');ribbon.addColorStop(1,'rgba(136,126,108,0)');ctx.fillStyle=ribbon;ctx.fill();
    ctx.beginPath();points.forEach(([x,y],i)=>i?ctx.lineTo(x,y):ctx.moveTo(x,y));ctx.strokeStyle='rgba(222,219,200,.3)';ctx.lineWidth=1.5;ctx.stroke();
    ctx.beginPath();points.forEach(([x,y],i)=>i?ctx.lineTo(x,y+11):ctx.moveTo(x,y+11));ctx.strokeStyle='rgba(210,204,186,.09)';ctx.lineWidth=4;ctx.stroke();
    for(let i=0;i<23;i++){const x=(i*503+33+t*3)%W,y=465+(i*97)%130+Math.sin(t*.27+i)*7;ctx.globalAlpha=fade*(.07+.12*Math.sin(i+t*.4)**2);ctx.fillStyle='#fff';ctx.fillRect(x,y,1.2,1.2);}
    ctx.restore();
    const reveal=ease((t-3.96)/.66);
    if(reveal){
      ctx.save(); ctx.globalAlpha=reveal; ctx.fillStyle='#f5f5f5';
      ctx.textAlign='center'; ctx.font='40px "KBPS3Startup", Arial, sans-serif';
      ctx.fillText('Kaneegabell Computer Entertainment',1190,500); ctx.restore();
    }
    const crest=Math.exp(-Math.pow((t-4.28)/.3,2))*.09*(.5+.5*energy);
    if(crest>0){ctx.fillStyle=`rgba(255,249,232,${crest})`;ctx.fillRect(0,0,W,H);}
  }
  function text(value,x,y,font,color,alpha=1) {
    ctx.save(); ctx.globalAlpha*=clamp(alpha); ctx.font=font; ctx.textAlign='center'; ctx.fillStyle=color;
    ctx.fillText(value,x,y); ctx.restore();
  }
  function glow(x,y,r,color,alpha) {
    ctx.save(); ctx.globalAlpha=clamp(alpha);
    const g=ctx.createRadialGradient(x,y,0,x,y,r); g.addColorStop(0,color); g.addColorStop(1,'rgba(0,0,0,0)');
    ctx.fillStyle=g; ctx.fillRect(x-r,y-r,r*2,r*2); ctx.restore();
  }
  function ps1(t) {
    const second=t>=6.2;
    ctx.fillStyle=second?'#000':'#b5b5b5'; ctx.fillRect(0,0,W,H);
    if(!second){
      const show=ease((t-.04)/.6)*(1-smooth((t-6.08)/.12));
      ctx.save(); ctx.globalAlpha=show;
      text('KANEEGABELL',CX,246,'700 67px Georgia, serif','#081e4a');
      ctx.save(); ctx.translate(CX,493);
      const g=ctx.createLinearGradient(-205,0,205,0); g.addColorStop(0,'#b60000'); g.addColorStop(.25,'#ce3b00'); g.addColorStop(.5,'#d6a500'); g.addColorStop(.75,'#c54a00'); g.addColorStop(1,'#bc0800');
      ctx.fillStyle=g; ctx.beginPath(); ctx.moveTo(0,-211); ctx.lineTo(205,0); ctx.lineTo(0,211); ctx.lineTo(-205,0); ctx.closePath(); ctx.fill();
      // The warm diamond keeps the original proportions; its emblem is KB's bull.
      const stamp=document.createElement('canvas');stamp.width=360;stamp.height=360;
      const c=stamp.getContext('2d');c.drawImage(marks.get('#e79c17'),0,0,360,360);c.globalCompositeOperation='source-in';
      const fold=c.createLinearGradient(0,0,360,0);fold.addColorStop(0,'#d6a500');fold.addColorStop(.4,'#d17000');fold.addColorStop(.55,'#ad1400');fold.addColorStop(1,'#d6a500');c.fillStyle=fold;c.fillRect(0,0,360,360);
      ctx.drawImage(stamp,-145.5,-145.5,291,291);ctx.restore();
      text('COMPUTER',CX,791,'400 53px Arial, sans-serif','#081e4a');
      text('ENTERTAINMENT',CX,839,'400 37px Arial, sans-serif','#081e4a'); ctx.restore();
    }else{
      const show=ease((t-6.32)/.08);
      ctx.save();ctx.globalAlpha=show;
      // Use KB's established artwork, with its original broken horn and gold ring.
      ctx.drawImage(brandImage,CX-128,247,256,256);
      text('Kaneegabell',CX,601,'76px "KBConsoleWordmark", Arial, sans-serif','#f5f5f5');
      text('Licensed by',CX,726,'400 29px Arial, sans-serif','#bfc0c2');
      text('Kaneegabell Computer Entertainment',CX,772,'400 29px Arial, sans-serif','#bfc0c2');
      ctx.restore();
    }
  }
  // Fixed values keep every export deterministic, including all dust and towers.
  const random = i => { const x=Math.sin(i*127.1+31.7)*43758.5453; return x-Math.floor(x); };
  function ps2(t) {
    ctx.fillStyle='#000';ctx.fillRect(0,0,W,H);
    if(t<7.6){
      const opacity=ease(t/.55)*(1-smooth((t-7.28)/.32));
      ctx.save();ctx.globalAlpha=opacity;
      glow(CX,445,820,'#193c7b',.85);glow(550,690,450,'#102c70',.55);
      // Low fog and an advancing camera through translucent rectangular towers.
      const flight=smooth((t-5.65)/1.35);
      const yaw=.12*Math.sin(t*.17)+flight*.22,camera=700-t*26-flight*400;
      const proj=([x,y,z])=>{const xx=x*Math.cos(yaw)-z*Math.sin(yaw),zz=x*Math.sin(yaw)+z*Math.cos(yaw)+camera;return [CX+xx*760/Math.max(100,zz),430+(y+320)*760/Math.max(100,zz)];};
      const towers=Array.from({length:48},(_,i)=>({x:(random(i+1)-.5)*3300,z:random(i+201)*2500,h:120+random(i+401)*650,w:46+random(i+601)*72})).sort((a,b)=>b.z-a.z);
      for(const {x,z,h,w} of towers){
        const polygons=[{v:[[x,0,z],[x+w,0,z],[x+w,-h,z],[x,-h,z]],c:'rgba(116,145,187,.33)'},{v:[[x+w,0,z],[x+w,0,z+w],[x+w,-h,z+w],[x+w,-h,z]],c:'rgba(165,180,204,.39)'},{v:[[x,-h,z],[x+w,-h,z],[x+w,-h,z+w],[x,-h,z+w]],c:'rgba(217,228,244,.7)'}];
        for(const {v,c} of polygons){ctx.beginPath();v.map(proj).forEach(([a,b],i)=>i?ctx.lineTo(a,b):ctx.moveTo(a,b));ctx.closePath();ctx.fillStyle=c;ctx.fill();}
      }
      for(let i=0;i<75;i++){const z=250+((random(i+801)*2300-t*60)%2300+2300)%2300;const [x,y]=proj([(random(i+901)-.5)*2500,-random(i+1001)*900,z]);glow(x,y,3+1200/z,'#9cceff',.3+.5*random(i));}
      const title=ease((t-.6)/.7)*(1-smooth((t-4.35)/.75));
      text('Kaneegabell Computer Entertainment',CX,527,'400 38px "KBPS3Startup", Arial, sans-serif','#f5f5f5',title);
      ctx.restore();
    }
    const reveal=smooth((t-7.71)/.16);
    if(reveal){
      ctx.save();ctx.globalAlpha=reveal;ctx.translate(CX,550);ctx.scale(1+(1-reveal)*.06,1+(1-reveal)*.06);
      ctx.shadowColor='#4364bb';ctx.shadowBlur=14;ctx.filter=`blur(${(1-reveal)*5}px)`;
      const ink=ctx.createLinearGradient(0,-65,0,15);ink.addColorStop(0,'#c1d3fc');ink.addColorStop(.5,'#f1f2ff');ink.addColorStop(1,'#778bd4');
      text('Kaneegabell',0,0,'78px "KBConsoleWordmark", Arial, sans-serif',ink);
      ctx.restore();
    }
  }
  function psp(t) {
    const white=smooth((t-3.56)/.24);
    ctx.fillStyle='#03030b';ctx.fillRect(0,0,W,H);
    glow(CX,1060,1000,'#343055',.45*(1-white));
    const waveFade=ease((t-.12)/.45)*(1-smooth((t-3.56)/.24));
    if(waveFade>0){
      ctx.save();ctx.globalAlpha=waveFade;
      const launch=ease((t-2.6)/.6);
      for(let band=0;band<5;band++){
        const phase=t*.55+band*.10;
        const g=ctx.createLinearGradient(0,0,W,0);g.addColorStop(0,'rgba(72,88,205,0)');g.addColorStop(.25,'rgba(78,101,224,.36)');g.addColorStop(.6,'rgba(133,83,226,.5)');g.addColorStop(1,'rgba(83,84,207,0)');
        ctx.beginPath();
        for(let i=0;i<=160;i++){
          const u=i/160,x=u*W,y=520+Math.sin(u*4.5+phase)*(12+launch*65)+Math.sin(u*2.9-phase)*launch*32+band*9;
          i?ctx.lineTo(x,y):ctx.moveTo(x,y);
        }
        ctx.strokeStyle=g;ctx.lineWidth=3+launch*(9+band*3);ctx.shadowColor=band<2?'#5868d8':'#7950bc';ctx.shadowBlur=8+launch*16;ctx.stroke();
      }
      ctx.restore();
    }
    const company=smooth((t-.18)/.35)*(1-smooth((t-2.35)/.35));
    text('Kaneegabell Computer Entertainment',CX,505,'37px "KBPS3Startup", Arial, sans-serif','#f5f5f5',company);
    if(white>0){ctx.fillStyle=`rgba(255,255,255,${white})`;ctx.fillRect(0,0,W,H);}
    // An entire waves-only stage precedes the white screen and logo reveal.
    const logo=smooth((t-3.812)/.24);
    if(logo>0){
      // Readable uppercase KB, in the portable logo's thin rectangular style.
      ctx.save();ctx.globalAlpha=logo;ctx.translate(CX,0);ctx.strokeStyle='#96999c';ctx.lineWidth=4;
      ctx.beginPath();ctx.moveTo(-170,400);ctx.lineTo(-170,523);ctx.moveTo(-40,400);ctx.lineTo(-170,465);ctx.lineTo(-40,523);
      ctx.moveTo(30,523);ctx.lineTo(30,400);ctx.lineTo(153,400);ctx.lineTo(153,459);ctx.lineTo(30,459);
      ctx.moveTo(153,459);ctx.lineTo(169,459);ctx.lineTo(169,523);ctx.lineTo(30,523);ctx.stroke();ctx.restore();
      text('Kaneegabell Computer Entertainment',CX,643,'42px "KBConsoleWordmark", Arial, sans-serif','#929497',logo);
    }
    const flare=smooth((t-3.85)/.08)*(1-smooth((t-4.12)/.30));
    if(flare>0){
      const x=720+smooth((t-3.85)/.55)*480,y=503;
      ctx.save();ctx.globalAlpha=flare;
      const gold=ctx.createRadialGradient(x,y,0,x,y,145);gold.addColorStop(0,'#fffef3');gold.addColorStop(.18,'rgba(255,244,182,.8)');gold.addColorStop(.55,'rgba(255,213,90,.25)');gold.addColorStop(1,'rgba(255,201,55,0)');ctx.fillStyle=gold;ctx.fillRect(x-145,y-145,290,290);
      const ray=ctx.createLinearGradient(x-300,0,x+300,0);ray.addColorStop(0,'rgba(255,216,105,0)');ray.addColorStop(.5,'rgba(255,231,161,.65)');ray.addColorStop(1,'rgba(255,216,105,0)');ctx.fillStyle=ray;ctx.fillRect(x-300,y-1,600,2);ctx.restore();
    }
  }
  function xbox360(t) {
    const base=ctx.createRadialGradient(CX,420,80,CX,490,1300);base.addColorStop(0,'#fff');base.addColorStop(1,'#dce5d8');ctx.fillStyle=base;ctx.fillRect(0,0,W,H);
    const resolve=smooth((t-1.13)/2.62);
    const radius=400-(resolve*255),cy=490-resolve*120;
    if(t>1.05 && t<4.25){
      const progress=clamp((t-1.05)/2.72),fade=1-smooth((t-3.77)/.48);
      ctx.save();ctx.globalAlpha=fade;
      for(let j=0;j<5;j++){
        const a=progress*Math.PI*3.5+j*.8;
        ctx.beginPath();for(let i=0;i<=125;i++){const u=i/125,theta=a+u*Math.PI*1.6,r=(1-u)*640+100;const x=CX+Math.cos(theta)*r,y=470+Math.sin(theta)*r*.52-(1-u)*150;i?ctx.lineTo(x,y):ctx.moveTo(x,y);}
        ctx.strokeStyle=['#73b900','#a1d430','#bde75d','#5eac10','#d2f381'][j];ctx.lineWidth=9+j*4;ctx.lineCap='round';ctx.shadowColor='#99d724';ctx.shadowBlur=15;ctx.stroke();
      }ctx.restore();
    }
    if(t>=1.13){
      ctx.save();ctx.globalAlpha=ease((t-1.13)/.45);
      // Lit metallic sphere, with the familiar green cuts curving over its surface.
      ctx.shadowColor='rgba(63,83,33,.22)';ctx.shadowBlur=32;ctx.shadowOffsetY=10;
      const sphere=ctx.createRadialGradient(CX-radius*.35,cy-radius*.4,5,CX,cy,radius);sphere.addColorStop(0,'#fff');sphere.addColorStop(.55,'#f7f8f5');sphere.addColorStop(.86,'#ccd0c8');sphere.addColorStop(1,'#929c8e');
      ctx.fillStyle=sphere;ctx.beginPath();ctx.arc(CX,cy,radius,0,Math.PI*2);ctx.fill();ctx.shadowBlur=0;ctx.shadowOffsetY=0;
      ctx.clip();
      ctx.save();ctx.translate(CX,cy);ctx.rotate((1-resolve)*.95);ctx.translate(-CX,-cy);
      mark(CX+radius*.015,cy+radius*.025,radius*1.75,'#315d12',.8);
      mark(CX,cy,radius*1.75,'#73b900');ctx.restore();ctx.restore();
      const title=smooth((t-3.63)/.24);
      tracking('KANEEGABELL',CX,670,'400 61px "Segoe UI", Arial, sans-serif',1.5,'#58a30b',title);
    }
    if(t>=3.75 && t<4.63){const u=(t-3.75)/.88;ctx.save();ctx.globalAlpha=(1-u)*.5;ctx.strokeStyle='#a1cf6a';ctx.lineWidth=5;ctx.beginPath();ctx.ellipse(CX,510,150+u*1500,60+u*560,0,0,Math.PI*2);ctx.stroke();ctx.restore();}
    const flash=Math.exp(-Math.pow((t-3.75)/.065,2))*.78;if(flash>.01){ctx.fillStyle=`rgba(255,255,255,${flash})`;ctx.fillRect(0,0,W,H);}
  }
  function switchIntro(t) {
    ctx.fillStyle='#e60012';ctx.fillRect(0,0,W,H);
    const drop=1-ease(t/.06),x=CX,y=400;
    ctx.save();ctx.translate(x,y);ctx.strokeStyle='#fff';ctx.lineWidth=18;ctx.fillStyle='#fff';
    ctx.beginPath();ctx.roundRect(-135,-140,120,250,[60,0,0,60]);ctx.stroke();
    ctx.beginPath();ctx.arc(-75,-63,24,0,Math.PI*2);ctx.fill();
    ctx.save();ctx.translate(0,-drop*150);ctx.beginPath();ctx.roundRect(15,-149,129,268,[0,65,65,0]);ctx.fill();
    ctx.fillStyle='#e60012';ctx.beginPath();ctx.arc(79,25,25,0,Math.PI*2);ctx.fill();ctx.restore();ctx.restore();
    text('KANEEGABELL',CX,659,'700 63px Arial, sans-serif','#fff');
  }
  function blend(a,b,p){
    const rgb=h=>[1,3,5].map(i=>parseInt(h.slice(i,i+2),16));const aa=rgb(a),bb=rgb(b);
    return `rgb(${aa.map((v,i)=>Math.round(v+(bb[i]-v)*p)).join(',')})`;
  }
  function konami(t) {
    const finish=smooth((t-2.13)/.74),gray=Math.round(255*finish);
    ctx.fillStyle=`rgb(${gray},${gray},${gray})`;ctx.fillRect(0,0,W,H);
    const ribbon=new Path2D('M-190 -58 H-70 C-45 -58 -35 -15 -10 -15 H180 V57 H65 C40 57 30 14 5 14 H-190 Z');
    for(let i=0;i<2;i++){
      const start=i?.79:.12,p=ease((t-start)/.65);
      if(p<=0)continue;
      ctx.save();ctx.translate(CX+(1-p)*(i?460:-460),390+(i?49:-49));
      ctx.transform(Math.max(.06,p),0,-.22,1,0,0);ctx.rotate((1-p)*(i?.5:-.5));
      const top=i?'#e92b22':'#ff9f1b',edge=i?'#8b1614':'#bc450b';
      ctx.save();ctx.translate(0,7);ctx.fillStyle=blend('#4b515e',edge,finish);ctx.fill(ribbon);ctx.restore();
      const g=ctx.createLinearGradient(-160,-58,150,57);g.addColorStop(0,blend('#7c8797',top,finish));g.addColorStop(.34,blend('#f4f6fa',top,finish));g.addColorStop(.58,blend('#bcc6d4',i?'#cc211b':'#ffb72c',finish));g.addColorStop(1,blend('#606b80',edge,finish));ctx.fillStyle=g;ctx.fill(ribbon);
      ctx.strokeStyle=blend('#e7ecf4',i?'#f34b37':'#ffc153',finish);ctx.lineWidth=1;ctx.stroke(ribbon);ctx.restore();
    }
    const name=ease((t-1.46)/.3);
    tracking('KANEEGABELL',CX,665,'italic 900 72px Arial, sans-serif',.5,blend('#cbd2dc','#353535',finish),name);
    const shine=clamp((t-2.26)/.48);
    if(shine>0&&shine<1)glow(600+shine*720,390,120,'#fff',Math.sin(shine*Math.PI)*.45);
  }
  function arcText(value,r,start,end,size,color) {
    ctx.save();ctx.font=`700 ${size}px "Arial Narrow", Arial, sans-serif`;ctx.fillStyle=color;ctx.textAlign='center';
    [...value].forEach((letter,i)=>{const angle=start+(end-start)*i/(value.length-1);ctx.save();ctx.rotate(angle);ctx.fillText(letter,0,-r);ctx.restore();});ctx.restore();
  }
  function shell(x,y,r,angle,light) {
    ctx.save();ctx.translate(x,y);ctx.rotate(angle);ctx.scale(1,.94);
    // The headstamp is drawn once per shell, with concentric machined metal rings.
    const metal=ctx.createLinearGradient(-r,-r,r,r);metal.addColorStop(0,'#aaa086');metal.addColorStop(.22,'#39362e');metal.addColorStop(.49,'#857457');metal.addColorStop(.67,'#28251e');metal.addColorStop(.88,'#a3926b');metal.addColorStop(1,'#171713');
    ctx.fillStyle=metal;ctx.beginPath();ctx.arc(0,0,r,0,Math.PI*2);ctx.fill();
    ctx.save();ctx.beginPath();ctx.arc(0,0,r,0,Math.PI*2);ctx.clip();
    for(let i=0;i<580;i++){const a=random(i+1701)*Math.PI*2,rr=Math.sqrt(random(i+1801))*r;ctx.fillStyle=i%2?'rgba(0,0,0,.13)':'rgba(224,204,164,.13)';ctx.fillRect(Math.cos(a)*rr,Math.sin(a)*rr,1+random(i)*3,1);}
    const red=ctx.createRadialGradient(-r*.7,r*.8,0,-r*.7,r*.8,r*1.25);red.addColorStop(0,'rgba(175,17,12,.6)');red.addColorStop(1,'rgba(0,0,0,0)');ctx.fillStyle=red;ctx.fillRect(-r,-r,r*2,r*2);ctx.restore();
    for(const [ratio,width,color] of [[.97,4,'#aaa08a'],[.91,5,'#171713'],[.87,2,'#6b604b'],[.55,4,'#14140f'],[.47,3,'#9c8d68'],[.34,4,'#171713']]){ctx.strokeStyle=color;ctx.lineWidth=width;ctx.beginPath();ctx.arc(0,0,r*ratio,0,Math.PI*2);ctx.stroke();}
    arcText('KANEEGABELL',r*.62,-1.05,1.05,r*.16,'#10100d');
    arcText('COMPUTER ENTERTAINMENT',r*.73,-2.05,-4.23,r*.10,'#10100d');
    text('KB',-r*.75,r*.09,`700 ${r*.15}px Arial`,'#100f0b');text('KB',r*.75,r*.09,`700 ${r*.15}px Arial`,'#100f0b');
    const primer=ctx.createRadialGradient(-r*.08,-r*.08,0,0,0,r*.3);primer.addColorStop(0,'#c2b797');primer.addColorStop(.35,'#39392d');primer.addColorStop(.69,'#867650');primer.addColorStop(1,'#11120e');ctx.fillStyle=primer;ctx.beginPath();ctx.arc(0,0,r*.3,0,Math.PI*2);ctx.fill();
    mark(0,0,r*.29,'#252525');
    ctx.fillStyle=`rgba(0,0,0,${1-light})`;ctx.beginPath();ctx.arc(0,0,r+3,0,Math.PI*2);ctx.fill();ctx.restore();
  }
  function rockstar(t) {
    ctx.fillStyle='#000';ctx.fillRect(0,0,W,H);
    const reveal=ease((t-1.10)/.45),closing=smooth((t-3.80)/.32);
    if(t<4.18){
      const zoom=1+ease((t-1.3)/1.5)*.12;
      ctx.save();ctx.translate(CX,520+closing*310);ctx.scale(zoom,zoom*(1-closing*.9));ctx.rotate(Math.sin(t*.5)*.014);
      ctx.fillStyle='#070807';ctx.beginPath();ctx.ellipse(0,0,578,321,0,0,Math.PI*2);ctx.fill();
      ctx.strokeStyle=`rgba(120,17,12,${reveal*.7})`;ctx.lineWidth=2;ctx.stroke();
      for(const x of [-246,246]){ctx.fillStyle='#030403';ctx.beginPath();ctx.arc(x,0,235,0,Math.PI*2);ctx.fill();shell(x,0,202,-.035+Math.sin(t*.8)*.012,reveal*.89);}
      const latch=ctx.createLinearGradient(-24,-300,30,-300);latch.addColorStop(0,'#111');latch.addColorStop(.5,'#a19f92');latch.addColorStop(1,'#151714');ctx.fillStyle=latch;ctx.fillRect(-25,-298,50,87);ctx.restore();
    }
    const after=t-5.08;
    if(after>=0 && after<1.8){
      const blast=Math.exp(-after*36);ctx.fillStyle=`rgba(255,235,199,${blast*.9})`;ctx.fillRect(0,0,W,H);
      for(let i=0;i<21;i++){const a=random(i+2401)*Math.PI*2,r=after*(80+random(i+2501)*150);glow(CX+Math.cos(a)*r,510+Math.sin(a)*r-after*70,55+after*75,'#9e9c91',(.065+random(i)*.035)*(1-after/1.8));}
    }
    const logo=smooth((t-5.22)/.38)*(1-smooth((t-10.5)/.4));
    mark(CX,469,281,'#d6241d',logo);
    tracking('KANEEGABELL',CX,736,'900 46px "Arial Black", Arial, sans-serif',1.5,'#d6241d',logo);
  }
  window.drawScene=(name,t,energy,duration)=>{
    ctx.globalAlpha=1;ctx.globalCompositeOperation='source-over';ctx.shadowBlur=0;
    if(name==='gba')gba(t);else if(name==='gamecube')gamecube(t);else if(name==='ps3')ps3(t,energy);
    else if(name==='ps1')ps1(t);else if(name==='ps2')ps2(t);else if(name==='psp')psp(t);else if(name==='xbox360')xbox360(t);
    else if(name==='switch')switchIntro(t);else if(name==='konami')konami(t);else if(name==='rockstar')rockstar(t);
    else throw new Error('Unknown classic scene '+name);
    if(!['gba','psp','switch','xbox360'].includes(name)){const fade=smooth((t-(duration-.2))/.2);if(fade){ctx.fillStyle=`rgba(0,0,0,${fade})`;ctx.fillRect(0,0,W,H);}}
  };
})();
