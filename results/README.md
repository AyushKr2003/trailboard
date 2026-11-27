# Results

Written by the bench workflow after every run. Chart: <https://ayushkr2003.github.io/trailboard/>

## Bench: Trailboard 8ad3a04 against 1 browsers

20261127-1615 · 3 runs each, medians · each case on its own runner · [workflow run](https://github.com/AyushKr2003/trailboard/actions/runs/46031642472)

✅ No regression against earlier runs on the same CPU.

Not run, by design (see browsers.toml): gnome-web (video-vp9, video-av1, video-h264).

### still

Runner: 4 × AMD EPYC 9V74 80-Core Processor, 15 GB; compared with 20261127-1538-9501932-46026446381.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 712 MB | 730 MB | 0.3 | 14 | 0.23 s | mem +1%, CPU +50% |
| chrome | Blink | 570 MB | 628 MB | 0.4 | 18 | 0.35 s | mem +2%, CPU +33% |

### video-vp9

Runner: 4 × AMD EPYC 7763 64-Core Processor, 15 GB; compared with 20261127-1348-5aa154c-46010069445.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 639 MB | 640 MB | 349.4 | 9 | 0.24 s | mem +1%, CPU -1% |
| chrome | Blink | 523 MB | 586 MB | 47.9 | 13 | 0.35 s | mem +5%, CPU -1% |

### video-h264

Runner: 4 × INTEL(R) XEON(R) PLATINUM 8573C, 15 GB; compared with 20261126-1507-e1edf21-45845659696.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 662 MB | 665 MB | 328.7 | 9 | 0.23 s | mem +5%, CPU -8% |
| chrome | Blink | 545 MB | 608 MB | 43.7 | 12 | 0.35 s | mem +6%, CPU -4% |

### pan

Runner: 4 × AMD EPYC 9V74 80-Core Processor, 15 GB; compared with 20261126-1507-e1edf21-45845659696.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 822 MB | 830 MB | 361.4 | 14 | 0.23 s | mem +4%, CPU -4% |

## Trailboard over time (newest first)

| Run | Trailboard | idle mem / CPU | still mem / CPU | video-vp9 mem / CPU | video-av1 mem / CPU | video-h264 mem / CPU | pan mem / CPU |
|---|---|---|---|---|---|---|---|
| [20261127-1615-8](https://github.com/AyushKr2003/trailboard/actions/runs/46031642472) | `8ad3a04` | – | 712 MB / 0.3 % | 639 MB / 349.4 % | – | 662 MB / 328.7 % | 822 MB / 361.4 % |
| [20261127-1554-a](https://github.com/AyushKr2003/trailboard/actions/runs/46028826700) | `a2d9617` | 583 MB / 0.2 % | 712 MB / 0.3 % | 631 MB / 228.6 % | 659 MB / 266.7 % | 644 MB / 256.8 % | 805 MB / 355.3 % |
| [20261127-1538-9](https://github.com/AyushKr2003/trailboard/actions/runs/46026446381) | `9501932` | 595 MB / 0.9 % | 703 MB / 0.2 % | – | – | 644 MB / 288.5 % | – |
| [20261127-1517-e](https://github.com/AyushKr2003/trailboard/actions/runs/46023384146) | `e6fe8cc` | 593 MB / 1.4 % | 711 MB / 0.6 % | – | – | 645 MB / 260.8 % | – |
| [20261127-1348-5](https://github.com/AyushKr2003/trailboard/actions/runs/46010069445) | `5aa154c` | 591 MB / 2.4 % | 690 MB / 0.8 % | 631 MB / 351.5 % | 654 MB / 258.9 % | 644 MB / 256.5 % | 814 MB / 348.0 % |
| [20261127-1223-5](https://github.com/AyushKr2003/trailboard/actions/runs/45998021086) | `5aa154c` | 579 MB / 3.7 % | 680 MB / 0.8 % | 623 MB / 269.9 % | 629 MB / 248.1 % | 642 MB / 248.5 % | 801 MB / 350.2 % |
| [20261127-1121-8](https://github.com/AyushKr2003/trailboard/actions/runs/45989683312) | `879f0c2` | 585 MB / 3.2 % | 704 MB / 0.7 % | 611 MB / 347.8 % | 646 MB / 332.3 % | 645 MB / 272.2 % | 812 MB / 352.6 % |
| [20261126-1507-e](https://github.com/AyushKr2003/trailboard/actions/runs/45845659696) | `e1edf21` | 574 MB / 4.7 % | 700 MB / 0.9 % | 591 MB / 366.7 % | 632 MB / 363.4 % | 631 MB / 358.9 % | 792 MB / 375.2 % |
| [20261126-0725-b](https://github.com/AyushKr2003/trailboard/actions/runs/45773585181) | `bd53a3d` | 578 MB / 4.7 % | 697 MB / 0.8 % | 619 MB / 366.2 % | 646 MB / 362.8 % | 646 MB / 364.3 % | 749 MB / 361.9 % |
| [20261126-0524-4](https://github.com/AyushKr2003/trailboard/actions/runs/45761289882) | `4c931f8` | 577 MB / 5.1 % | 699 MB / 0.6 % | 622 MB / 365.8 % | 672 MB / 376.2 % | 651 MB / 363.0 % | 746 MB / 368.2 % |
| [20261126-0418-e](https://github.com/AyushKr2003/trailboard/actions/runs/45754226156) | `ea5b59f` | 582 MB / 45.6 % | 697 MB / 0.5 % | 617 MB / 365.2 % | 648 MB / 372.2 % | 643 MB / 365.4 % | 738 MB / 358.0 % |
| [20261125-1734-e](https://github.com/AyushKr2003/trailboard/actions/runs/45665317928) | `ea5b59f` | 570 MB / 37.3 % | 684 MB / 0.6 % | 603 MB / 366.1 % | 640 MB / 364.1 % | 639 MB / 372.5 % | 742 MB / 359.7 % |
