# awecut

**awecut** (short for *awesome cut*) is a simple and powerful command-line tool to cut TV recordings with precision
— without reencoding. It’s built in Rust for speed and reliability, and focuses on making cutting effortless, 
whether you're using manual cue-points or want automated ad detection.

## ✂️ What It Does/Will do

- Cut TV recordings precisely at cue-points
- No reencoding – output is as fast and clean as possible
- Cuts are aligned to keyframes to preserve stream integrity
- Support for automatic ad/commercial detection using multiple methods

## 🔍 Why awecut?

***Recording your favorite shows is great — but watching them with commercials? Not so much.***

**awecut** aims to be the missing link in your TV recording workflow:

- Use it manually with your own cue-points
- Or let it do the work for you by detecting ads using advanced techniques
- Get a clean, reassembled version of your recording in no time

### 🔍 Why awecut in France?

French television is known to broadcast a lot of contents, including popular movies. I believe 
it is one of the greatest way to create an important library of TV shows, series and movies for free
at the cost of waiting a year after the cinema release and a reduced visual quality (but still better than DVD).
Most of Internet providers enable DRM-free recodings of most TNT-available channels (and obviously all 
contents received by the TNT is DRM-free), you just have to accept the commercials, but not anymore.

## 🚀 Features (Planned / In Progress)

- [ ] Automatic ad detection:
    - [ ] Black frame detection
    - [ ] Audio fingerprinting
    - [ ] Logo presence detection
    - [ ] Heuristic-based timing models
- [ ] Command-line interface (CLI)
- [ ] Cue-point-based cutting (with keyframe alignment)
- [ ] Fast stream copying (no reencoding)
- [ ] Multiple formats support (MPEG-TS, MP4, MKV, etc.)
- [ ] Timeline preview/export to popular editing formats (like EDL, CSV)

## 🛠 Example Usage

### Cut using predefined cue-points (Planned)

```bash
awecut cut --input recording.ts --cue-points cues.txt --output cut.ts
```


### Detect ads automatically and cut them out (In development)

```bash
awecut detect-and-cut --input recording.ts --output clean.ts
```

### 👷‍♂️ Development

To build from source:

```bash
git clone https://github.com/fusetim/awecut.git
cd awecut
cargo build --release
```

### 📄 License

Copyright 2025 - FuseTim

awecut, this program is free software: you can redistribute it and/or modify it under the terms of 
the GNU General Public License as published by the Free Software Foundation, either version 3 of the
License, or (at your option) any later version.

This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without 
even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General
Public License for more details.

You should have received a copy of the GNU General Public License along with this program. If not,
see <https://www.gnu.org/licenses/>. 