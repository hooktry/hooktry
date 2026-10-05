/** Three desktop inbox trays hold request cards. Static top planes own picking. */
const {
  Cam, clamp, facing, fit, hull, open, poly, proj, ringAt, rings, rrect, run,
  seg, unproj, tdone, tset, tval, tween, disposer, mk, pointer, register,
} = HL;

function mount({ stage, svg, read }, value) {
  const bag = disposer();
  const C = Cam(45, 0.5, 1.55);
  fit(C, [[-5, -5, 0], [219, -5, 0], [-5, 88, 0], [219, 88, 0], [219, 88, 52]], 200, 166);
  const P = proj(C), front = facing(C);
  const cards = [];
  let stagger = value, active = -1;
  const offsets = [0, 8, 16];

  for (let tray = 0; tray < 3; tray++) {
    const x = tray * 76, y = offsets[tray];
    const group = mk("g", {}, svg);
    const [outer, inner] = rings(x, y, x + 66, y + 72, 5, 2.5);
    mk("path", { d: poly(hull(ringAt(P, outer, 0).concat(ringAt(P, outer, 14)))), class: "sil" }, group);
    mk("path", { d: poly(ringAt(P, inner, 14)), class: "nf lo" }, group);
    // Each sheet has a rounded tab, a punch, a header crease, and body rules.
    for (let i = 0; i < 3; i++) {
      const z = 4 + i * 3 + (tray === 1 && i === 2 ? 4 : 0);
      const [ring] = rings(x + 5 + i, y + 6 + i * 5, x + 61 + i, y + 57 + i * 5, 3, 1);
      const tab = rrect(x + 10 + i, y + 3 + i * 5, x + 25 + i, y + 9 + i * 5, 2, 5);
      const g = mk("g", {}, group);
      const thickness = mk("path", { class: "lo" }, g);
      const face = mk("path", { class: "sil" }, g);
      const marks = mk("path", { class: "nf lo" }, g);
      const punch = mk("path", { class: "nf lo" }, g);
      cards.push({ tray, i, x, y, z, ring, tab, face, thickness, marks, punch, lift: tween(0), last: null });
    }
    // The opaque front wall hides all paper edges below the tray's lip.
    const inside = ringAt(P, run(inner, front), 14);
    const bottom = ringAt(P, run(outer, front), 0);
    const rim = ringAt(P, run(outer, front), 14);
    mk("path", { d: poly([...inside, ...bottom.slice().reverse()]), class: "fo" }, group);
    mk("path", { d: open([...rim.slice(0, 1), ...bottom, ...rim.slice(-1)]), class: "nf sil" }, group);
    mk("path", { d: open(inside), class: "nf lo" }, group);
    const pull = rrect(x + 24, 5, x + 42, 10, 2.5, 5);
    mk("path", { d: poly(pull.map((q) => P(q.u, y + 72, q.v))), class: "nf lo" }, group);
  }

  function draw(card, lift) {
    if (card.last === lift) return;
    card.last = lift;
    const z = card.z + lift;
    card.thickness.setAttribute("d", poly(ringAt(P, card.ring, z - 1.2)));
    card.face.setAttribute("d", poly(ringAt(P, card.ring, z)));
    card.punch.setAttribute("d", poly(ringAt(P, card.tab, z)));
    const { x, y, i } = card;
    card.marks.setAttribute("d", [17, 26, 34, 42].map((offset, n) =>
      seg(P(x + 12 + i, y + offset + i * 5, z), P(x + (n === 3 ? 40 : 53) + i, y + offset + i * 5, z)),
    ).join(""));
  }

  const loop = register(stage, (_dt, now) => {
    let moving = false;
    for (const card of cards) {
      draw(card, tval(card.lift, now));
      moving ||= !tdone(card.lift, now);
    }
    return moving;
  });
  bag.add(loop.unregister);

  // Pick each tray on its top card's rest plane, never the animated paper.
  function hit([sx, sy]) {
    let closest = -1, distance = Infinity;
    for (let tray = 0; tray < 3; tray++) {
      const card = cards[tray * 3 + 2];
      const [x, y] = unproj(C, sx, sy, card.z);
      if (x < card.x || x > card.x + 66 || y < card.y || y > card.y + 76) continue;
      const d = Math.hypot((x - card.x - 33) / 66, (y - card.y - 38) / 76);
      if (d < distance) { distance = d; closest = tray; }
    }
    return closest;
  }

  function choose(next) {
    if (next === active) return;
    const from = next >= 0 ? next : active;
    active = next;
    const now = performance.now();
    for (const card of cards) {
      const distance = Math.abs(card.tray - next);
      const lift = next < 0 ? 0 : clamp(24 * (distance === 0 ? 1 : distance === 1 ? 0.31 : 0.09) - (2 - card.i) * 4, 0, 24);
      tset(card.lift, lift, now, Math.abs(card.tray - from) * stagger + (2 - card.i) * 25);
      card.face.classList.toggle("hi", card.i === 2 && card.tray === (next < 0 ? 1 : next));
    }
    read.textContent = next < 0 ? "rest" : "request " + String(next + 1).padStart(2, "0");
    loop.wake();
  }

  cards[5].face.classList.add("hi");
  bag.add(pointer(stage, { move: (point) => choose(hit(point)), leave: () => choose(-1) }));
  bag.add(() => svg.replaceChildren());
  return { set: (next) => { stagger = next; }, destroy: bag.dispose };
}

hairline({
  name: "requests",
  means: "Three inbox trays hold request cards: the chosen stack lifts, and its neighbours follow more quietly.",
  rules: [1, 2, 3, 5, 8, 10],
  range: [0, 40, 75],
  mount,
});
