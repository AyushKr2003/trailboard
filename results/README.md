# Results

Written by the bench workflow after every run. Chart: <https://ayushkr2003.github.io/trailboard/>

## Bench: Trailboard 7248406 against 5 browsers

20261127-1712 · 3 runs each, medians · each case on its own runner · [workflow run](https://github.com/AyushKr2003/trailboard/actions/runs/46039758109)

✅ No regression against earlier runs on the same CPU.

Not run, by design (see browsers.toml): gnome-web (video-vp9, video-av1, video-h264).

### idle

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261127-1554-a2d9617-46028826700.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 594 MB | 595 MB | 0.9 | 9 | 0.21 s | mem +2%, CPU +350% |
| falkon | Blink (Qt WebEngine) | 402 MB | 402 MB | 9.6 | 5 | 0.21 s | mem -1%, CPU -1% |
| gnome-web | WebKit | 433 MB | 436 MB | 0.0 | 8 | 0.31 s | mem -0% |
| chrome | Blink | 438 MB | 496 MB | 0.6 | 12 | 0.22 s | mem +0%, CPU -14% |
| brave | Blink | 468 MB | 484 MB | 0.9 | 12 | 0.32 s | mem +3%, CPU +6% |
| firefox | Gecko | 742 MB | 849 MB | 4.7 | 10 | 0.82 s | mem -1%, CPU +16% |

### still

Runner: 4 × AMD EPYC 7763 64-Core Processor, 15 GB; compared with 20261127-1121-879f0c2-45989683312.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 700 MB | 703 MB | 0.3 | 14 | 0.32 s | mem -1%, CPU -57% |
| falkon | Blink (Qt WebEngine) | 425 MB | 443 MB | 0.2 | 5 | 0.32 s | mem -1%, CPU -33% |
| chrome | Blink | 534 MB | 590 MB | 0.4 | 18 | 0.45 s | mem +0%, CPU +0% |
| brave | Blink | 559 MB | 586 MB | 0.1 | 17 | 0.44 s | mem -0%, CPU +0% |
| firefox | Gecko | 804 MB | 883 MB | 7.4 | 16 | 1.27 s | mem -0%, CPU +45% |
| gnome-web | WebKit | 938 MB | 1107 MB | 0.1 | 38 | 0.42 s | mem -0%, CPU -50% |

### video-vp9

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261127-1631-d95458b-46034126404.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 625 MB | 626 MB | 220.1 | 9 | 0.21 s | mem +0%, CPU -2% |
| falkon | Blink (Qt WebEngine) | 490 MB | 496 MB | 185.0 | 5 | 0.21 s |  |
| chrome | Blink | 495 MB | 555 MB | 37.7 | 12 | 0.22 s | mem -5%, CPU -5% |
| brave | Blink | 514 MB | 522 MB | 37.1 | 12 | 0.32 s |  |
| firefox | Gecko | 828 MB | 913 MB | 47.3 | 11 | 0.83 s |  |

### video-av1

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261127-1554-a2d9617-46028826700.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 657 MB | 659 MB | 218.8 | 9 | 0.21 s | mem -0%, CPU -18% |
| chrome | Blink | 530 MB | 589 MB | 34.3 | 12 | 0.32 s | mem +0%, CPU -6% |
| brave | Blink | 550 MB | 554 MB | 33.0 | 12 | 0.32 s | mem -0%, CPU -7% |
| falkon | Blink (Qt WebEngine) | 586 MB | 618 MB | 183.6 | 5 | 0.21 s | mem -4%, CPU -2% |
| firefox | Gecko | 877 MB | 948 MB | 49.6 | 11 | 0.82 s | mem +1%, CPU -2% |

### video-h264

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261127-1631-d95458b-46034126404.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 652 MB | 653 MB | 221.2 | 9 | 0.21 s | mem +0%, CPU +5% |
| falkon | Blink (Qt WebEngine) | 507 MB | 513 MB | 182.3 | 5 | 0.21 s |  |
| chrome | Blink | 518 MB | 579 MB | 35.2 | 12 | 0.23 s | mem -5%, CPU +11% |
| brave | Blink | 537 MB | 543 MB | 32.8 | 12 | 0.32 s |  |
| firefox | Gecko | 842 MB | 918 MB | 47.4 | 11 | 0.83 s |  |

### pan

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261127-1348-5aa154c-46010069445.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 805 MB | 812 MB | 335.3 | 14 | 0.22 s | mem -1%, CPU -4% |

## Trailboard over time (newest first)

| Run | Trailboard | idle mem / CPU | still mem / CPU | video-vp9 mem / CPU | video-av1 mem / CPU | video-h264 mem / CPU | pan mem / CPU |
|---|---|---|---|---|---|---|---|
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
