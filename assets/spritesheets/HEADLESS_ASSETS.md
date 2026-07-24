# Headless character body assets

The playable character atlases now have non-destructive headless body variants
for a layered customization system similar to Ragnarok Online.

## File layout

Every character directory contains:

- `<role>_headless.png`: 1024 x 1024 production body atlas
- `<role>_headless_<direction>_walk.png`: eight directional strips
- `<role>_headless_<direction>_walk.gif`: eight animated previews

Directions remain:

`up`, `up_left`, `left`, `down_left`, `down`, `down_right`, `right`,
`up_right`

Each atlas remains an 8-column x 8-row grid of 128 x 128 cells. The head
socket and all background pixels have zero alpha and zero RGB values.

Headless variants exist for:

- Chasqui
- Shaman
- Quipucamayoc
- Amauta
- Haravicu
- Curaca
- Willac Umu
- Aclla
- Qollqa kamayuq
- Chacra kamayuq
- Llama michiq
- Mitmaq
- Yana
- Awqaq
- Runa simi kamayuq

## Runtime layering

Use the same atlas cell index and transform for each layer:

1. headless class body
2. base head or face
3. hairstyle
4. lower headgear, if used
5. upper headgear, if used

Future head, hair, and headgear atlases should use the same 1024 x 1024
canvas, 8 x 8 grid, animation timing, and per-cell anchor as the body atlas.

## Rebuilding

Run `scripts/create_headless_sprites.ps1` from the repository root to rebuild
the headless master atlases from the original character PNGs. The script:

- preserves original source files
- ignores sparse generated pixels that spill across row boundaries
- tracks profile-view head positions
- preserves off-center staffs, shields, and tools
- clears head pixels to RGBA `(0, 0, 0, 0)`

The image-generation edit workflow was used as a prototype with this request:

> Remove only the entire head layer from every sprite—face, hair, ears,
> headband, feathers, and headgear—while preserving every body pixel, atlas
> cell, pose, and piece of class equipment. Leave a clean transparent
> neck-level socket for separately layered hairstyles and headgear.

The generative prototype did not preserve the invariant reliably, so the
production assets use the deterministic alpha-mask script instead.
