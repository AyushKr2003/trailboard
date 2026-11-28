# Results

Written by the bench workflow after every run. Chart: <https://ayushkr2003.github.io/trailboard/>

## Bench: Trailboard 3bea198 against 5 browsers

20261128-0654 · 3 runs each, medians · each case on its own runner · [workflow run](https://github.com/AyushKr2003/trailboard/actions/runs/46118301029)

✅ No regression against earlier runs on the same CPU (not checked, no earlier run on their CPU: idle).

Not run, by design (see browsers.toml): gnome-web (video-vp9, video-av1, video-h264).

### idle

Runner: 4 × Intel(R) Xeon(R) Platinum 8370C CPU @ 2.80GHz, 15 GB; no earlier run on this CPU to compare with.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 598 MB | 600 MB | 0.2 | 9 | 0.32 s |  |
| falkon | Blink (Qt WebEngine) | 409 MB | 410 MB | 13.3 | 5 | 0.21 s |  |
| gnome-web | WebKit | 433 MB | 436 MB | 0.0 | 8 | 0.42 s |  |
| chrome | Blink | 438 MB | 495 MB | 0.8 | 12 | 0.33 s |  |
| brave | Blink | 467 MB | 477 MB | 1.3 | 12 | 0.33 s |  |
| firefox | Gecko | 746 MB | 832 MB | 6.3 | 10 | 1.15 s |  |

### still

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261127-1554-a2d9617-46028826700.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 705 MB | 722 MB | 0.3 | 14 | 0.21 s | mem -1%, CPU +0% |
| falkon | Blink (Qt WebEngine) | 430 MB | 445 MB | 0.3 | 5 | 0.21 s | mem -1%, CPU +50% |
| chrome | Blink | 540 MB | 589 MB | 0.3 | 18 | 0.34 s | mem +2%, CPU +0% |
| brave | Blink | 564 MB | 581 MB | 0.1 | 17 | 0.32 s | mem +0%, CPU +0% |
| firefox | Gecko | 817 MB | 971 MB | 4.8 | 16 | 0.83 s | mem +1%, CPU +0% |
| gnome-web | WebKit | 952 MB | 1008 MB | 0.1 | 38 | 0.31 s | mem -6%, CPU +0% |

### video-vp9

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261127-1738-576f9ed-46043186873.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 624 MB | 639 MB | 223.4 | 9 | 0.21 s | mem +1%, CPU -1% |
| chrome | Blink | 496 MB | 557 MB | 38.1 | 12 | 0.22 s | mem -0%, CPU -5% |
| falkon | Blink (Qt WebEngine) | 499 MB | 500 MB | 178.5 | 5 | 0.21 s | mem +1%, CPU -10% |
| brave | Blink | 516 MB | 523 MB | 36.2 | 12 | 0.33 s | mem -1%, CPU -6% |
| firefox | Gecko | 829 MB | 922 MB | 47.5 | 11 | 0.83 s | mem -1%, CPU -0% |

### video-av1

Runner: 4 × INTEL(R) XEON(R) PLATINUM 8573C, 15 GB; compared with 20261127-1121-879f0c2-45989683312.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 656 MB | 659 MB | 278.7 | 9 | 0.32 s | mem +2%, CPU -16% |
| chrome | Blink | 531 MB | 589 MB | 45.4 | 12 | 0.33 s | mem +1%, CPU -5% |
| brave | Blink | 549 MB | 557 MB | 43.3 | 12 | 0.33 s | mem +4%, CPU -6% |
| falkon | Blink (Qt WebEngine) | 586 MB | 610 MB | 239.3 | 5 | 0.32 s | mem +3%, CPU -8% |
| firefox | Gecko | 852 MB | 925 MB | 65.1 | 11 | 1.14 s | mem -0%, CPU -4% |

### video-h264

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261127-1738-576f9ed-46043186873.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 646 MB | 647 MB | 223.5 | 9 | 0.21 s | mem +2%, CPU +2% |
| falkon | Blink (Qt WebEngine) | 511 MB | 518 MB | 192.5 | 5 | 0.21 s | mem -0%, CPU +7% |
| chrome | Blink | 518 MB | 578 MB | 35.0 | 12 | 0.33 s | mem +0%, CPU +2% |
| brave | Blink | 540 MB | 543 MB | 33.6 | 12 | 0.32 s | mem +0%, CPU +1% |
| firefox | Gecko | 837 MB | 913 MB | 48.5 | 11 | 0.83 s | mem -0%, CPU +2% |

### pan

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261127-1712-7248406-46039758109.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 804 MB | 812 MB | 335.7 | 14 | 0.22 s | mem -0%, CPU +0% |

## Trailboard over time (newest first)

| Run | Trailboard | idle mem / CPU | still mem / CPU | video-vp9 mem / CPU | video-av1 mem / CPU | video-h264 mem / CPU | pan mem / CPU |
|---|---|---|---|---|---|---|---|
| [20261128-0654-3](https://github.com/AyushKr2003/trailboard/actions/runs/46118301029) | `3bea198` | 598 MB / 0.2 % | 705 MB / 0.3 % | 624 MB / 223.4 % | 656 MB / 278.7 % | 646 MB / 223.5 % | 804 MB / 335.7 % |
| [20261127-1738-5](https://github.com/AyushKr2003/trailboard/actions/runs/46043186873) | `576f9ed` | 585 MB / 0.8 % | 703 MB / 0.3 % | 617 MB / 226.4 % | 649 MB / 224.0 % | 633 MB / 219.0 % | 817 MB / 342.1 % |
| [20261127-1712-7](https://github.com/AyushKr2003/trailboard/actions/runs/46039758109) | `7248406` | 594 MB / 0.9 % | 700 MB / 0.3 % | 625 MB / 220.1 % | 657 MB / 218.8 % | 652 MB / 221.2 % | 805 MB / 335.3 % |
| [20261127-1631-d](https://github.com/AyushKr2003/trailboard/actions/runs/46034126404) | `d95458b` | – | – | 624 MB / 224.0 % | – | 651 MB / 211.0 % | 795 MB / 351.2 % |
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
