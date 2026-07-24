# Inca role character roster

All production atlases use the same engine contract:

- 1024 x 1024 RGBA PNG with a transparent background
- 8 columns x 8 rows
- 128 x 128 pixels per cell
- columns: eight distinct walk-cycle frames
- rows: `up`, `up_left`, `left`, `down_left`, `down`, `down_right`,
  `right`, `up_right`

Each generated character directory contains:

- `<role>.png`: production atlas
- eight `<role>_<direction>_walk.png` directional strips
- eight `<role>_<direction>_walk.gif` animated previews

## Roles and paths

| Role | Production atlas | Visual design cue |
| --- | --- | --- |
| Chasqui | `chasqui/chasqui.png` | Existing relay messenger asset |
| Quipucamayoc | `quipucamayoc/quipucamayoc.png` | Compact quipu bundle and record pouch |
| Amauta | `amauta/amauta.png` | Refined mantle, headband, and teaching baton |
| Haravicu | `haravicu/haravicu.png` | Panpipe, compact drum, and rhythmic posture |
| Curaca | `curaca/curaca.png` | Fine unku, mantle, accounting pouch, ceremonial staff |
| Willac Umu | `willac_umu/willac_umu.png` | Formal dark-and-cream robes, sun pectoral, ritual staff |
| Aclla | `aclla/aclla.png` | Anaku, lliclla, tupu pins, spindle, and yarn |
| Qollqa kamayuq | `qollqa_kamayuq/qollqa_kamayuq.png` | Inventory quipu, grain pouch, and tally cloth |
| Chacra kamayuq | `chacra_kamayuq/chacra_kamayuq.png` | Seed pouch and compact digging stick |
| Llama michiq | `llama_michiq/llama_michiq.png` | Highland poncho, rope coil, sling, and herding staff |
| Mitmaq | `mitmaq/mitmaq.png` | Travel bundle, seed pouch, and practical field tool |
| Yana | `yana/yana.png` | Simple maintained clothing, folded cloth, and serving vessel |
| Awqaq | `awqaq/awqaq.png` | Quilted tunic, shield, and star-headed macana |
| Runa simi kamayuq | `runa_simi_kamayuq/runa_simi_kamayuq.png` | Conversational gesture, reference quipu, and travel pouch |

`character_roster_preview.png` shows the first frame of the front-facing row
for all fourteen roles in the table order.

## Generation prompt set

The generated roles used the existing Chasqui atlas as a strict style, scale,
camera-angle, and layout reference. Each request used this common production
prompt with the role-specific design cue from the table:

> Create a new playable character class named `<role>`, grounded respectfully
> in Inca-era Andean material culture and visually distinguished by
> `<role-specific design cue>`. Match the reference's hand-authored pixel-art
> style, sprite scale, proportions, camera angle, density, and atlas
> organization without copying the Chasqui design. Produce exactly 64
> consistent full-body sprites on an invisible 8-column by 8-row grid: eight
> successive 45-degree viewing directions by eight coherent seamless
> walk-cycle frames. Keep one centered sprite inside every 128-by-128 cell with
> a consistent foot baseline and no boundary crossings. Use hard pixel edges,
> a limited woven-textile palette, selective outlines, and no antialiasing,
> painterly rendering, or 3D. Include no modern objects, generic
> pan-Indigenous styling, extra characters, animals, scenery, text, labels,
> watermark, shadows, blank cells, or duplicated directions. Generate against
> a perfectly uniform removable magenta chroma key; do not use magenta in the
> character.

The built-in image-generation workflow produced the keyed sources. The
project-local deliverables were converted to alpha, resized with
nearest-neighbor sampling, and validated cell by cell. The Runa simi kamayuq
source returned seven directions; its missing rear diagonal was assembled from
a mirrored compatible rear-direction row before final validation.
