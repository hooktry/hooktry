# Hairline theme

Reference: https://hairline.lucasmarkes.com/inspo

The working interface uses a white or near-black canvas, fine borders, restrained
monochrome controls, and Hairline's interactive line figures. `/theme` is a separate
concept homepage and illustration bench, reachable from the left navigation and
the mobile menu. Navigating there keeps the current Hook, stream, search and
inspector selection in memory. The concept's sample request is explicitly example
data; its Try a Hook button uses the existing creation flow.

## Figures

`@lucasmarkes/hairline` is pinned to 0.2.0. Riffle, Dish, Slow, Terminal and Patch are the
upstream React components, with accessible descriptions and reduced-motion support.
The empty request list uses Dish, a receiving antenna that aims toward the pointer
or touch. It is removed when the first actual request arrives and is absent from
filtered no-match states. The landing page keeps its Riffle illustration.

`requests.js` is an original figure made using the upstream
[hairline-create skill](https://github.com/lucasmarkes/hairline/tree/main/skills/hairline-create).
The chosen metaphor is three desktop inbox trays: the pointer lifts request cards
and its neighbours follow with less reach. Picking uses each top card's static rest
plane. The highlights move to exactly one top card, and the loop sleeps once settled.

`../public/hairline-requests.html` is the standalone deliverable. The upstream kernel
and bench are embedded without modification. Its theme and intensity controls work
offline; the Theme page embeds this same file. Source and engine are MIT licensed;
the upstream notice is preserved in `../public/hairline-LICENSE.txt`.

To rebuild with the upstream skill folder available:

```sh
node /path/to/hairline-create/build.mjs apps/web/design/requests.js apps/web/public/hairline-requests.html
node /path/to/hairline-create/validate.mjs apps/web/public/hairline-requests.html
node /path/to/hairline-create/look.mjs apps/web/public/hairline-requests.html --answer 109,56,14 --edge 203,60,10
```

The skill validator passes with its kernel and bench intact. All eight visual
checks stay inside the 400 by 320 frame, show the correct read-out, and report no
console warnings or errors. The drawing settles within 1.8 seconds. The sheet was
inspected for legibility at 240 px, paint order, rounded corners, one highlight,
both themes, and the absence of words in the SVG.

This branch is an independent alternative to PR #257 and starts from main at
`fa6724e5972cbc1f88a2c568000efcdefcf01790`.
