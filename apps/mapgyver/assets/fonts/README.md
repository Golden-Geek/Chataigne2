# Calibration font

MapGyver embeds the unmodified Noto Sans Mono Regular font and its SIL Open Font License
notice from [notofonts/noto-fonts](https://github.com/notofonts/noto-fonts/tree/ffebf8c1ee449e544955a7e813c54f9b73848eac).
Upstream file: `hinted/ttf/NotoSansMono/NotoSansMono-Regular.ttf`.

- Font SHA-256: `d9e2b23d19f8230be7146f409a52b1d23117e635e28f2e2892cf91b7382f325b`
- License SHA-256: `0dab92d0544f7b233403f14b84a663bdbfa746982eda629e7f4f9ffe1b036feb`

The preparation worker rasterizes the outlines at the requested calibration density, then
packs readout glyphs, smaller cell-label glyphs and the logo into one admitted image. Live
values select existing glyphs on the render worker; changing a clock value never rasterizes
text or creates another texture. The bundle includes `OFL.txt` beside the executable resources.
