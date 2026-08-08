# Latest character sprites

This is the clean, flat production export. It contains only the latest
transparent individual-animation sheets for all 29 characters in male and
female versions.

There are no intermediate files, legacy atlases, obsolete attack sheets, or
nested character directories.

## Filename format

```text
<character>-<gender>-<animation>.png
```

Example:

```text
haravicu-female-walk.png
haravicu-female-attack1.png
haravicu-female-attack2.png
```

## Animations per character

Each character and gender has:

- `walk`
- `idle`
- `sit`
- `death`
- `pickup`
- `cast`
- `hit`
- `attack1` -- sword-compatible, front and back, 7 frames each
- `attack2` -- spear-compatible, front and back, 6 frames each

All sheets use transparent backgrounds and fixed 128 x 128 frame cells. Heads
and weapons remain excluded because they are separate sprite layers.

`manifest.json` lists every character variant and its exact filenames.
