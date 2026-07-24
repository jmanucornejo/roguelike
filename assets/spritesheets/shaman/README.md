# Shaman sprite assets

`shaman.png` is the production atlas:

- Canvas: 1024 x 1024 RGBA
- Grid: 8 columns x 8 rows
- Cell size: 128 x 128
- Columns: eight walk-cycle frames
- Rows: `up`, `up_left`, `left`, `down_left`, `down`, `down_right`, `right`, `up_right`

The directional PNG strips and animated GIFs are derived from the production
atlas. They are previews and editing aids; the game should load `shaman.png`.

## Generation

Generated with the built-in image generation workflow using
`../chasqui/chasqui.png` as a strict style, scale, and atlas-layout reference.
The generated chroma-key source was converted to alpha and resized with
nearest-neighbor sampling.

Final prompt:

> Create a new playable character class called the Shaman, clearly distinct
> from the Chasqui but matching the reference's exact pixel-art style, scale,
> proportions, camera angle, sprite density, and animation-atlas organization.
> Use a respectful Andean-inspired fantasy design: layered woven poncho, ritual
> headband, modest feather or leaf ornaments, medicine pouches, charms, and a
> short carved ceremonial staff. Use ochre, deep brown, cream, muted red,
> turquoise, and violet. Produce exactly 64 consistent full-body sprites on a
> precise invisible 8-by-8 grid: eight successive 45-degree directions by
> eight coherent seamless walk-cycle frames. Every cell is 128 by 128, with a
> consistent foot baseline and no boundary crossings, blank cells, labels,
> text, watermark, large spell effects, shadows, or extra characters. Render
> as crisp hand-authored 2D pixel art with hard pixel edges, a limited palette,
> subtle selective outlines, and no antialiasing, painterly shading, or 3D
> rendering. Use a uniform removable magenta chroma-key background and never
> use magenta in the character.
