# Results

Written by the bench workflow after every run. Chart: <https://ayushkr2003.github.io/trailboard/>

## Bench: Trailboard 9501932 against 1 browsers

20261127-1538 · 3 runs each, medians · each case on its own runner · [workflow run](https://github.com/AyushKr2003/trailboard/actions/runs/46026446381)

✅ No regression against earlier runs on the same CPU (not checked, no earlier run on their CPU: video-h264).

Not run, by design (see browsers.toml): gnome-web (video-vp9, video-av1, video-h264).

### idle

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261127-1517-e6fe8cc-46023384146.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 595 MB | 596 MB | 0.9 | 9 | 0.22 s | mem +0%, CPU -36% |
| chrome | Blink | 463 MB | 522 MB | 0.6 | 12 | 0.24 s | mem -0%, CPU +0% |

### still

Runner: 4 × AMD EPYC 9V74 80-Core Processor, 15 GB; compared with 20261127-1517-e6fe8cc-46023384146.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 703 MB | 705 MB | 0.2 | 14 | 0.23 s | mem -1%, CPU -67% |
| chrome | Blink | 559 MB | 617 MB | 0.3 | 18 | 0.35 s | mem +0%, CPU +0% |

### video-h264

Runner: 4 × AMD EPYC 9V74 80-Core Processor, 15 GB; no earlier run on this CPU to compare with.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 644 MB | 645 MB | 288.5 | 9 | 0.23 s |  |
| chrome | Blink | 545 MB | 605 MB | 36.3 | 13 | 0.35 s |  |

## Trailboard over time (newest first)

| Run | Trailboard | idle mem / CPU | still mem / CPU | video-vp9 mem / CPU | video-av1 mem / CPU | video-h264 mem / CPU | pan mem / CPU |
|---|---|---|---|---|---|---|---|
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
