The two-frame PNG (APNG) and lossless WebP are generated test fixtures, not user
images. Both contain two 5×3 RGBA frames: `(200, 80, 40, 128)` then
`(0, 0, 255, 128)`, delays 40/90 ms, and two total plays. APNG uses SOURCE blend
and NONE disposal. Reproduce with Pillow 12.1.1:

```python
from PIL import Image
a = Image.new('RGBA', (5, 3), (200, 80, 40, 128))
b = Image.new('RGBA', (5, 3), (0, 0, 255, 128))
a.save('two-frame.png', save_all=True, append_images=[b], duration=[40, 90],
       loop=2, blend=0, disposal=0)
a.save('two-frame.webp', save_all=True, append_images=[b], duration=[40, 90],
       loop=2, lossless=True, exact=True)
```

Pillow is only needed to regenerate these fixtures. Rust tests decode the checked
in bytes using the production codecs; installed applications do not need Python.

`release-two-frame.png` and `release-two-frame.webp` have the same pixels and
two total plays, with 500/500-ms frame delays. Reproduce the two save calls above
with `duration=[500, 500]`. The installed-release AT-SPI checks use the longer dwell
to operate the actual Replay/Pause/Play controls without racing a 40-ms frame.

`two-frame.gif` uses the same dimensions/delays but opaque red/blue frames, with
`loop=2` (two repetitions after the first play, hence three total plays):

```python
a = Image.new('RGBA', (5, 3), (255, 0, 0, 255))
b = Image.new('RGBA', (5, 3), (0, 0, 255, 255))
a.save('two-frame.gif', save_all=True, append_images=[b], duration=[40, 90], loop=2)
a.save('two-frame-infinite.gif', save_all=True, append_images=[b], duration=[40, 90], loop=0)
```

`over-budget.gif` has 129 alternating opaque frames and exercises readable still
preview behavior beyond the 128-frame timeline limit:

```python
frames = [Image.new('RGB', (5, 3), 'red' if n % 2 == 0 else 'blue')
          for n in range(129)]
frames[0].save('over-budget.gif', save_all=True, append_images=frames[1:],
               duration=40, loop=0)
```
