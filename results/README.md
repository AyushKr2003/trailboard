# Results

Written by the bench workflow after every run. Chart: <https://ayushkr2003.github.io/trailboard/>

## Bench: Trailboard a2d9617 against 5 browsers

20261127-1554 · 3 runs each, medians · each case on its own runner · [workflow run](https://github.com/AyushKr2003/trailboard/actions/runs/46028826700)

✅ No regression against earlier runs on the same CPU (not checked, no earlier run on their CPU: video-vp9, pan).

Not run, by design (see browsers.toml): gnome-web (video-vp9, video-av1, video-h264).

### idle

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261127-1538-9501932-46026446381.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 583 MB | 584 MB | 0.2 | 9 | 0.21 s | mem -2%, CPU -78% |
| falkon | Blink (Qt WebEngine) | 406 MB | 413 MB | 9.7 | 5 | 0.21 s |  |
| gnome-web | WebKit | 435 MB | 438 MB | 0.0 | 8 | 0.32 s |  |
| chrome | Blink | 438 MB | 494 MB | 0.7 | 12 | 0.22 s | mem -5%, CPU +17% |
| brave | Blink | 454 MB | 474 MB | 0.9 | 12 | 0.32 s |  |
| firefox | Gecko | 748 MB | 852 MB | 4.1 | 10 | 0.82 s |  |

### still

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261127-1348-5aa154c-46010069445.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 712 MB | 728 MB | 0.3 | 14 | 0.21 s | mem +3%, CPU -62% |
| falkon | Blink (Qt WebEngine) | 434 MB | 446 MB | 0.2 | 5 | 0.21 s | mem +1%, CPU -33% |
| chrome | Blink | 532 MB | 593 MB | 0.3 | 18 | 0.34 s | mem +1%, CPU +0% |
| brave | Blink | 562 MB | 577 MB | 0.1 | 17 | 0.32 s | mem +1%, CPU -50% |
| firefox | Gecko | 810 MB | 911 MB | 4.8 | 16 | 0.83 s | mem -1%, CPU +41% |
| gnome-web | WebKit | 1018 MB | 1128 MB | 0.1 | 38 | 0.32 s | mem +3%, CPU +0% |

### video-vp9

Runner: 4 × AMD EPYC 9V45 96-Core Processor, 15 GB; no earlier run on this CPU to compare with.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 631 MB | 633 MB | 228.6 | 9 | 0.21 s |  |
| chrome | Blink | 497 MB | 559 MB | 32.0 | 13 | 0.33 s |  |
| falkon | Blink (Qt WebEngine) | 498 MB | 498 MB | 173.8 | 5 | 0.21 s |  |
| brave | Blink | 522 MB | 528 MB | 29.6 | 12 | 0.32 s |  |
| firefox | Gecko | 834 MB | 906 MB | 36.4 | 11 | 0.93 s |  |

### video-av1

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261127-1348-5aa154c-46010069445.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 659 MB | 662 MB | 266.7 | 9 | 0.21 s | mem +1%, CPU +3% |
| chrome | Blink | 530 MB | 589 MB | 36.3 | 12 | 0.32 s | mem +0%, CPU +6% |
| brave | Blink | 552 MB | 554 MB | 35.5 | 12 | 0.32 s | mem +0%, CPU +7% |
| falkon | Blink (Qt WebEngine) | 608 MB | 618 MB | 188.1 | 5 | 0.21 s | mem +3%, CPU +5% |
| firefox | Gecko | 865 MB | 943 MB | 50.7 | 11 | 0.83 s | mem +0%, CPU +1% |

### video-h264

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261127-1517-e6fe8cc-46023384146.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 644 MB | 646 MB | 256.8 | 9 | 0.21 s | mem -0%, CPU -2% |
| falkon | Blink (Qt WebEngine) | 511 MB | 516 MB | 175.4 | 5 | 0.21 s |  |
| chrome | Blink | 518 MB | 578 MB | 33.1 | 12 | 0.22 s | mem -5%, CPU -2% |
| brave | Blink | 539 MB | 543 MB | 31.9 | 12 | 0.32 s |  |
| firefox | Gecko | 841 MB | 932 MB | 46.8 | 11 | 0.83 s |  |

### pan

Runner: 4 × AMD EPYC 9V45 96-Core Processor, 15 GB; no earlier run on this CPU to compare with.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 805 MB | 814 MB | 355.3 | 14 | 0.23 s |  |

## Trailboard over time (newest first)

| Run | Trailboard | idle mem / CPU | still mem / CPU | video-vp9 mem / CPU | video-av1 mem / CPU | video-h264 mem / CPU | pan mem / CPU |
|---|---|---|---|---|---|---|---|
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
