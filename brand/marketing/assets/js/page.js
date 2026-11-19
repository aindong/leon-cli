// The landing page's behaviour: the menu on a narrow screen, the test run that plays in leon's window,
// a pause control for every loop, and the statement's ripple under the pointer. The page is whole
// without it: every line and picture is in the HTML, and the run shows its finished state.
const still = matchMedia('(prefers-reduced-motion: reduce)').matches;

// the menu button opens the bar's words as a sheet; a tap on a word, Esc or the button closes it
const toggle = document.querySelector('.nav-toggle');
if (toggle) {
  const set = (open) => {
    document.documentElement.classList.toggle('menu-open', open);
    toggle.setAttribute('aria-expanded', String(open));
  };
  toggle.addEventListener('click', () => set(toggle.getAttribute('aria-expanded') !== 'true'));
  document.querySelector('.nav-links').addEventListener('click', (e) => { if (e.target.closest('a')) set(false); });
  addEventListener('keydown', (e) => { if (e.key === 'Escape') set(false); });
}

// a pause button for each loop: it stops the run and the diagram's light where they are
for (const host of document.querySelectorAll('[data-loop]')) {
  const button = document.createElement('button');
  button.type = 'button';
  button.className = 'pause';
  button.setAttribute('aria-label', 'Pause the animation');
  button.innerHTML = '<svg class="hold" viewBox="0 0 12 12" aria-hidden="true"><path d="M3 2h2v8H3zM7 2h2v8H7z" fill="currentColor"/></svg><svg class="play" viewBox="0 0 12 12" aria-hidden="true"><path d="M3 2l7 4-7 4z" fill="currentColor"/></svg>';
  button.addEventListener('click', () => {
    const paused = host.classList.toggle('paused');
    button.setAttribute('aria-label', paused ? 'Play the animation' : 'Pause the animation');
  });
  if (!still) host.append(button);
}

// the test run: the command is typed again, each suite ticks in turn, then the summary and a fresh
// prompt; one cycle is 3.2 s, and it runs only while it is on screen and not paused
const run = document.querySelector('[data-run]');
if (run && !still) {
  const host = run.closest('[data-loop]');
  const typed = run.querySelector('[data-cmd]');
  const command = typed.textContent;
  const outs = [...run.querySelectorAll('[data-out]')];
  const steps = outs.filter((el) => el.hasAttribute('data-step'));
  const bar = run.querySelector('.bar');
  let visible = false, timer = 0, at = 0;
  // the program: [ms after the cycle starts, what happens]
  const program = [];
  const typeEnd = 60 * command.length;
  for (let i = 0; i <= command.length; i++) program.push([60 * i, () => { typed.textContent = command.slice(0, i); }]);
  program.push([typeEnd + 120, () => { outs[0].hidden = false; }]);
  steps.forEach((step, i) => {
    const t = typeEnd + 300 + i * 360;
    program.push([t, () => { step.hidden = false; step.classList.add('busy'); }]);
    program.push([t + 220, () => { step.classList.remove('busy'); bar.style.setProperty('--p', (i + 1) / steps.length); }]);
  });
  const tail = outs.slice(outs.indexOf(steps[steps.length - 1]) + 1);
  tail.forEach((el, i) => program.push([typeEnd + 300 + steps.length * 360 + i * 140, () => { el.hidden = false; }]));
  const cycle = 3200;
  const reset = () => {
    for (const el of outs) el.hidden = true;
    for (const step of steps) step.classList.remove('busy');
    bar.style.setProperty('--p', 0);
    typed.textContent = '';
  };
  const tick = () => {
    if (!visible || host.classList.contains('paused')) { timer = 0; return; }
    if (at === 0) reset();
    for (const [t, act] of program) if (t >= at && t < at + 40) act();
    at = (at + 40) % cycle;
    timer = setTimeout(tick, 40);
  };
  new IntersectionObserver(([entry]) => {
    visible = entry.isIntersecting;
    if (visible && !timer) tick();
  }).observe(run);
  host.addEventListener('click', (e) => { if (e.target.closest('.pause') && !timer) tick(); });
}

// the statement ripples where the pointer crosses it and settles back to crisp type within a second:
// one SVG displacement filter, on only while the ripple lives
const word = document.querySelector('[data-ripple]');
if (word && !still && matchMedia('(hover: hover) and (pointer: fine)').matches) {
  const ns = 'http://www.w3.org/2000/svg';
  const svg = document.createElementNS(ns, 'svg');
  svg.setAttribute('aria-hidden', 'true');
  svg.style.cssText = 'position:absolute;width:0;height:0';
  svg.innerHTML = '<filter id="ripple" x="-5%" y="-20%" width="110%" height="140%"><feTurbulence type="fractalNoise" baseFrequency="0.006 0.013" numOctaves="1" seed="4"/><feDisplacementMap in="SourceGraphic" scale="0" xChannelSelector="R" yChannelSelector="G"/></filter>';
  document.body.append(svg);
  const noise = svg.querySelector('feTurbulence'), shift = svg.querySelector('feDisplacementMap');
  let energy = 0, frame = 0, last = null;
  const draw = (now) => {
    energy *= 0.9;
    if (energy < 0.6) { word.style.filter = ''; frame = 0; return; }
    shift.setAttribute('scale', Math.min(40, energy).toFixed(1));
    noise.setAttribute('baseFrequency', `0.006 ${(0.013 + Math.sin(now / 700) * 0.003).toFixed(4)}`);
    word.style.filter = 'url(#ripple)';
    frame = requestAnimationFrame(draw);
  };
  word.addEventListener('pointermove', (e) => {
    if (last) energy = Math.min(60, energy + Math.hypot(e.clientX - last[0], e.clientY - last[1]) * 0.35);
    last = [e.clientX, e.clientY];
    if (!frame) frame = requestAnimationFrame(draw);
  });
  word.addEventListener('pointerleave', () => { last = null; });
}
