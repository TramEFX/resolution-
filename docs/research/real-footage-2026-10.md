# Real footage through the engine (first pass, October 2026)

The research report's risk #6 says FilmCraft's decoders are bit-exact on conformance streams, but
real camera and phone files are "less tested". This pass ran real files from the development
machine through Resolution's engine (FilmCraft v0.6.0, pin `9acf816`) on Windows 11. **What it can
and cannot say:** the machine held almost no camera originals, so the set is mostly downloads and
re-encodes. It covers the awkward cases listed below, but not iPhone HEVC/HDR, 10-bit, GoPro,
mirrorless or ProRes footage. That is the next pass.

## Method

1. **Survey:** `ffprobe` (FFmpeg 9.0.2) read 127 video files (40 MP4 + 1 MKV in Videos, 84 MP4 + 2
   MOV in Downloads) and grouped them into **33 technical profiles** (source, container, codec and
   profile, pixel format, size, frame rate, audio track count, rotation).
2. **Engine:** for one file per profile, a fresh `resolution mcp` server did the following:
   imported it (`file.import`), read `file.mediaProperties`, built a timeline from it
   (`file.newSequenceFromClip`), and rendered frames at 10 %, 50 % and 90 % (`render_frame`, 640 px).
3. **Reference:** ffmpeg decoded each time point at the same size. SSIM and PSNR compared each
   pair, and side-by-side images settled the low scores.
4. **Timing:** to tell a wrong frame from a neighbouring one, three synthetic H.264 MP4s with a
   unique picture per frame (`testsrc`, with a 3-decimal timer) were rendered at known times:
   constant 24 fps, the same clip starting 53 ms late, and variable frame rate.

## Results

| Check | Result |
|---|---|
| Import | **33/33 profiles**; no import errors, no crashes |
| Timeline from clip | 33/33 |
| Frames rendered | **99/99** |
| H.264 Baseline / Main / High, `yuv420p` and full-range `yuvj420p` | decoded; full-range levels match ffmpeg (checked by eye) |
| HEVC Main, including 3840×2160 at 60 fps | decoded |
| 5 audio tracks (2560×1070 game captures) | **all 5 found**, one timeline track each |
| Rotation tag (Android, −90°) | **applied**: renders portrait, upright, matching ffmpeg |
| Odd sizes (1916×1080, 1260×720, 480×486, 464×688), MKV, MOV, files without audio | decoded |
| Variable frame rate (synthetic) | **exact**: the frame on screen at each test time, including inside a 7-frame hold |
| Constant frame rate (synthetic) | **exact** at 4/4 test times when frames sit exactly on the grid; one frame late when they sit just after it (B-011) |
| Stream starting 53 ms late (synthetic, and 2 real files) | **one frame early (LOG B-010)** |

Most low similarity scores in the first comparison came from the reference, not the engine.
`ffmpeg -ss t` returns the first frame *at or after* t, while the engine correctly shows the frame
*on screen* at t. Measured against the right frame, constant-rate files agree within one frame
interval. Resampling of fine text accounts for the rest: the game captures score PSNR ~20 on
visually identical frames.

### Findings

- **B-010, open:** an MP4 whose video edit list starts with an empty edit, then a B-frame skip
  edit, plays one frame early. In the synthetic clip (`elst` = 53 ms empty, then 10 000 ms at media
  time 1024 of 12 288), FilmCraft shows timer **2.000** (frame 48) at 2.1042 s. The file's
  presentation timeline puts frame 49 there; counting from the clip's first frame, it is frame 50.
  Without the empty edit, FilmCraft is exact. Real files: 2 of 127 start late (53 ms, 67 ms). One
  measured −2.0 / −2.1 frames against the zero-based expectation, which matches.
- **Frame-rate label for variable-rate files:** `mediaProperties` and the timeline built from the
  clip use an estimated rate (58.824 for a 59.98-average capture, 37.5 for a 32.06-average MOV).
  Frames are still placed by their timestamps (exact in the synthetic test). The visible effect is
  a timeline at an unusual rate. Cosmetic, but an editor would notice.

## Android 4K clip (second pass, 2026-10-12)

A phone original arrived after the first pass: Android 15, **3840×2160 H.264 High at level 5.1,
~48 Mbit/s**, 30 fps nominal (953 of 958 frames 1/30 s, five jittered), 8-bit BT.709, AAC stereo,
no B-frames, no edit list.

| Check | Result |
|---|---|
| Import, metadata | correct: codec, 3840×2160, 30.01 fps (the measured average), 4:2:0 8-bit, BT.709, 31.921 s, AAC 2 ch 48 kHz |
| Timeline from clip | 1 video and 1 audio clip |
| Random seeks, 640 px preview | 10/10, **0.28 to 0.92 s** each |
| Full 4K frame | rendered; **45.8 dB** against ffmpeg's decode of the same frame |
| 30 consecutive frames, 640 px | **~17 frames/s** (median 28 ms) |
| Frame timing | **one frame late at 8 of 10 times (B-011)** |

The speed figures come from a development build (codec crates at opt-level 3), and each frame also
pays for an MCP round trip and PNG encoding. Treat them as rough indicators, not playback speed.
They do suggest 4K30 playback needs the multi-threaded read-ahead FilmCraft keeps in its UI crate.

**B-011, open:** the engine shows the last source frame at or before the start of the timeline
frame. That rule predicts 10/10 measurements on this clip; "the frame on screen" predicts 2/10.
Frames that sit fractions of a millisecond after the grid therefore show one frame late. A synthetic
clip with every frame 1 ms late shows 0.000 / 2.466 / 4.966 where 0.033 / 2.500 / 5.000 belong. The
first pass's Android 720p clip (−1.1 to −1.6 frames) fits the same pattern.

## Camera originals (third pass, 2026-10-12)

Two originals, kept outside the repo. The camera and phone were identified from the files' own
metadata, which differed from what the files had been labelled as:

- **Sony α7R II** (`ILCE-7RM2` in the file's XAVC metadata, brand `XAVC`): XAVC S, 3840×2160 H.264
  High at level 5.1, **~100 Mbit/s**, 23.976 fps constant (every frame 1001/24000 s), B-frames,
  8-bit with no colour tags, **big-endian 16-bit PCM** stereo 48 kHz, a Sony real-time metadata
  track (`rtmd`), start timecode 02:01:12:00. 3 min 40 s, 2.8 GB.
- **Samsung phone** (Android 15, `com.samsung.android` tags): 4K H.264 High 5.1 at ~48 Mbit/s,
  stored sideways with a **−90° rotation tag** (portrait), ~30 fps (the first frame 45.8 ms, the
  rest 1/30 s), BT.709, AAC stereo 48 kHz. 28 s.

| Check | Sony α7R II | Samsung portrait |
|---|---|---|
| Import, metadata | correct, including PCM 16-bit and the start timecode 02:01:12:00 | correct: 2160×3840 after rotation, AAC; rate labelled 30.01 (first pass's estimated-rate label) |
| Timeline from clip | 3840×2160 at 23.976, 1 video + 1 audio | 2160×3840 at 30.01, 1 video + 1 audio |
| Frame on screen at 10/50/90 % | **exact, 3 of 3** (PSNR 34.6 to 35.1 dB against that frame) | the B-011 rule's frame at 3 of 3; **one frame late at 2 of them** (B-011) |
| Playback, hardware decoding (`play_bench`, 1920 px) | **24.0 fps**, every display refresh the exact frame, at 20 s and 150 s | **24.6 to 26.1 fps** of 30 (after B-015's fix; before it, a deadlock) |
| Playback, software decoding | 23.2 to 23.9 fps | **freezes** after the preroll (B-017) |
| Real editor window, sound on the M-Track (`--play 8`) | **24.0 fps**, 0 underruns | 18.6 fps, 0 underruns |
| Sound against ffmpeg's decode (`play_bench --wav`, 12 s) | **sample-exact**: 0 samples of offset, gain 1.0000, difference 149 dB down; the same at 20 s and 3 min 25 s, so no drift | **sample-exact** (AAC): 0 offset, gain 1.0000, 136 dB down |

The Android clip from the second pass gives the same sound result (0 offset, 136 dB down). The
sound comparison covers what the simulated device received through the play-ahead ring at 1x.
Picture and sound are therefore each where the file puts them, except B-011's frame-late picture on
phone footage. **Watched and listened to** in the editor by Truman on 2026-10-13 (both clips, sound
on the M-Track): frame rates consistent with the source, sound good.

**Findings:**

- **B-015 (fixed):** two frame workers deadlocked on the rotated Samsung clip, a bug in Resolution's
  own playback (the B-014 fix). Found only because this clip was rotated.
- **B-017 (open):** with software decoding the Samsung clip freezes, although the decoder alone
  manages 15.6 fps in order.
- The Samsung clip shows 18.6 fps in the editor's window against 24.6 to 26.1 in `play_bench`. Not
  investigated; the window may request a larger picture than the benchmark's 1920 px.
- No new decoding defects: the 100 Mbit/s Sony stream, big-endian PCM and the extra metadata track
  all decode correctly.

## Not covered yet

- **Camera and phone originals:** iPhone HEVC (HDR / Dolby Vision, 10-bit), Android HEVC, GoPro,
  10-bit 4:2:2 mirrorless (long-GOP and All-I), ProRes, MTS/AVCHD, Blackmagic RAW. The third pass
  covered one 8-bit mirrorless camera and a second phone. This is the pass that decides how soon a
  fallback decoder is needed.
- **Audio:** decoded samples and sync are now checked for three files (PCM and AAC, stereo). Not yet:
  other codecs, channel layouts beyond stereo, files whose audio starts later than the video.
- **Other platforms:** Windows only.

The harness (Python + WSL ffmpeg) lives outside the repo for now. It can become a repeatable
check once the camera-original corpus exists.
