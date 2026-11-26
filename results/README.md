# Results

Written by the bench workflow after every run. Chart: <https://ayushkr2003.github.io/trailboard/>

## Bench: Trailboard bd53a3d against 16 browsers

20261126-0725 · 3 runs each, medians · each case on its own runner · [workflow run](https://github.com/AyushKr2003/trailboard/actions/runs/45773585181)

✅ No regression against earlier runs on the same CPU (not checked, no earlier run on their CPU: video-av1).

Failed to run: chromium, gnome-web, opera (see the logs).

### idle

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261126-0524-4c931f8-45761289882.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 578 MB | 578 MB | 4.7 | 9 | 0.21 s | mem +0%, CPU -8% |
| palemoon | Goanna | 257 MB | 267 MB | 0.5 | 1 | 0.52 s | mem -2%, CPU -17% |
| falkon | Blink (Qt WebEngine) | 393 MB | 401 MB | 9.8 | 5 | 0.21 s | mem -1%, CPU -3% |
| ungoogled-chromium | Blink | 428 MB | 440 MB | 0.7 | 10 | 0.22 s | mem -0%, CPU +0% |
| qutebrowser | Blink (Qt WebEngine) | 430 MB | 443 MB | 0.0 | 5 | 0.52 s | mem +2% |
| chrome | Blink | 435 MB | 492 MB | 0.6 | 13 | 0.22 s | mem -1%, CPU -25% |
| gnome-web | WebKit | 437 MB | 439 MB | 0.0 | 8 | 0.31 s | mem +1% |
| brave | Blink | 468 MB | 485 MB | 0.7 | 12 | 0.33 s | mem +0%, CPU -22% |
| edge | Blink | 494 MB | 606 MB | 0.5 | 12 | 0.33 s | mem -0%, CPU -17% |
| chromium | Blink | 502 MB | 544 MB | 0.8 | 12 | 0.22 s | mem +1%, CPU -11% |
| opera | Blink | 549 MB | 656 MB | 6.2 | 14 | 0.32 s | mem -0%, CPU -18% |
| waterfox | Gecko | 568 MB | 981 MB | 8.7 | 10 | 1.23 s | mem +0%, CPU -2% |
| floorp | Gecko | 654 MB | 812 MB | 10.6 | 9 | 0.83 s | mem +1%, CPU -12% |
| zen | Gecko | 664 MB | 858 MB | 16.4 | 11 | 0.83 s | mem -1%, CPU -1% |
| vivaldi | Blink | 688 MB | 862 MB | 5.0 | 12 | 0.33 s | mem -0%, CPU -22% |
| firefox | Gecko | 750 MB | 843 MB | 3.3 | 10 | 0.82 s | mem -0%, CPU -35% |
| librewolf | Gecko | 995 MB | 1276 MB | 2.6 | 9 | 0.93 s | mem +1%, CPU -10% |

### still

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261126-0524-4c931f8-45761289882.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 697 MB | 699 MB | 0.8 | 14 | 0.21 s | mem -0%, CPU +33% |
| palemoon | Goanna | 206 MB | 209 MB | 0.1 | 1 | 0.52 s | mem -1%, CPU +0% |
| falkon | Blink (Qt WebEngine) | 435 MB | 447 MB | 0.3 | 5 | 0.21 s | mem +2%, CPU +0% |
| ungoogled-chromium | Blink | 527 MB | 539 MB | 0.3 | 15 | 0.34 s | mem -0%, CPU +50% |
| chrome | Blink | 529 MB | 586 MB | 0.4 | 18 | 0.33 s | mem -1%, CPU +33% |
| brave | Blink | 529 MB | 548 MB | 0.1 | 17 | 0.32 s | mem -6%, CPU +0% |
| qutebrowser | Blink (Qt WebEngine) | 560 MB | 574 MB | 0.0 | 10 | 0.52 s | mem -1% |
| edge | Blink | 587 MB | 709 MB | 0.2 | 17 | 0.43 s | mem +0%, CPU -33% |
| chromium | Blink | 598 MB | 645 MB | 0.5 | 17 | 0.33 s | mem -1%, CPU +0% |
| opera | Blink | 658 MB | 790 MB | 0.6 | 20 | 0.33 s | mem -1%, CPU -14% |
| floorp | Gecko | 664 MB | 856 MB | 2.7 | 15 | 0.93 s | mem +0%, CPU +13% |
| waterfox | Gecko | 677 MB | 1063 MB | 9.6 | 16 | 1.36 s | mem +0%, CPU +4% |
| zen | Gecko | 754 MB | 934 MB | 19.6 | 17 | 0.93 s | mem -0%, CPU +17% |
| firefox | Gecko | 814 MB | 906 MB | 5.2 | 16 | 0.95 s | mem -0%, CPU +4% |
| vivaldi | Blink | 864 MB | 1013 MB | 1.5 | 18 | 0.45 s | mem +0%, CPU +7% |
| gnome-web | WebKit | 967 MB | 978 MB | 0.1 | 38 | 0.31 s | mem -5%, CPU +0% |
| librewolf | Gecko | 1047 MB | 1308 MB | 3.0 | 15 | 0.94 s | mem -4%, CPU +7% |

### video-vp9

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261126-0524-4c931f8-45761289882.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 619 MB | 623 MB | 366.2 | 9 | 0.21 s | mem -0%, CPU +0% |
| palemoon | Goanna | 262 MB | 268 MB | 31.8 | 1 | 0.52 s | mem +0%, CPU +6% |
| qutebrowser | Blink (Qt WebEngine) | 467 MB | 470 MB | 199.2 | 5 | 0.52 s | mem +2%, CPU +1% |
| ungoogled-chromium | Blink | 492 MB | 503 MB | 40.6 | 10 | 0.22 s | mem +1%, CPU -3% |
| chrome | Blink | 494 MB | 553 MB | 40.4 | 12 | 0.32 s | mem -1%, CPU +3% |
| falkon | Blink (Qt WebEngine) | 497 MB | 505 MB | 187.1 | 5 | 0.21 s | mem +0%, CPU -2% |
| brave | Blink | 516 MB | 523 MB | 37.6 | 12 | 0.33 s | mem +0%, CPU +1% |
| edge | Blink | 543 MB | 655 MB | 40.0 | 12 | 0.33 s | mem -0%, CPU +2% |
| chromium | Blink | 555 MB | 598 MB | 40.0 | 12 | 0.33 s | mem -1%, CPU +7% |
| opera | Blink | 604 MB | 724 MB | 41.8 | 15 | 0.32 s | mem +0%, CPU -2% |
| waterfox | Gecko | 703 MB | 1068 MB | 48.6 | 11 | 1.24 s | mem +0%, CPU +2% |
| floorp | Gecko | 716 MB | 870 MB | 44.6 | 10 | 0.93 s | mem +0%, CPU +2% |
| vivaldi | Blink | 768 MB | 931 MB | 51.6 | 13 | 0.44 s | mem -1%, CPU -1% |
| zen | Gecko | 813 MB | 1018 MB | 66.5 | 12 | 0.93 s | mem +1%, CPU -1% |
| firefox | Gecko | 833 MB | 907 MB | 48.6 | 11 | 0.83 s | mem +0%, CPU +0% |
| librewolf | Gecko | 1150 MB | 1326 MB | 47.4 | 10 | 0.93 s | mem +2%, CPU +4% |
| gnome-web | WebKit | failed: video not playing (page said cannot-play, playing; windows:  | | | | | |

### video-av1

Runner: 4 × INTEL(R) XEON(R) PLATINUM 8573C, 15 GB; no earlier run on this CPU to compare with.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 646 MB | 648 MB | 362.8 | 9 | 0.32 s |  |
| palemoon | Goanna | 290 MB | 295 MB | 43.8 | 1 | 0.63 s |  |
| ungoogled-chromium | Blink | 524 MB | 531 MB | 47.4 | 10 | 0.33 s |  |
| chrome | Blink | 528 MB | 586 MB | 46.3 | 12 | 0.33 s |  |
| brave | Blink | 549 MB | 552 MB | 43.9 | 12 | 0.33 s |  |
| qutebrowser | Blink (Qt WebEngine) | 563 MB | 586 MB | 252.1 | 5 | 0.63 s |  |
| edge | Blink | 575 MB | 686 MB | 46.5 | 12 | 0.44 s |  |
| chromium | Blink | 594 MB | 635 MB | 46.8 | 12 | 0.33 s |  |
| falkon | Blink (Qt WebEngine) | 615 MB | 630 MB | 234.0 | 5 | 0.32 s |  |
| opera | Blink | 636 MB | 745 MB | 46.5 | 15 | 0.34 s |  |
| waterfox | Gecko | 746 MB | 1097 MB | 65.3 | 11 | 1.66 s |  |
| floorp | Gecko | 750 MB | 838 MB | 58.2 | 10 | 1.25 s |  |
| vivaldi | Blink | 806 MB | 963 MB | 59.6 | 13 | 0.56 s |  |
| zen | Gecko | 847 MB | 1033 MB | 86.7 | 12 | 1.14 s |  |
| firefox | Gecko | 869 MB | 925 MB | 63.6 | 11 | 1.14 s |  |
| librewolf | Gecko | 1464 MB | 1511 MB | 59.6 | 10 | 1.15 s |  |
| gnome-web | WebKit | failed: video not playing (page said cannot-play, playing; windows:  | | | | | |

### video-h264

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261126-0524-4c931f8-45761289882.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 646 MB | 648 MB | 364.3 | 9 | 0.21 s | mem -1%, CPU +0% |
| palemoon | Goanna | 273 MB | 281 MB | 30.7 | 1 | 0.52 s | mem +0%, CPU -4% |
| qutebrowser | Blink (Qt WebEngine) | 488 MB | 488 MB | 181.7 | 5 | 0.52 s | mem +0%, CPU -8% |
| ungoogled-chromium | Blink | 512 MB | 522 MB | 34.9 | 10 | 0.22 s | mem +0%, CPU -0% |
| chrome | Blink | 517 MB | 579 MB | 33.6 | 12 | 0.23 s | mem -2%, CPU -2% |
| falkon | Blink (Qt WebEngine) | 517 MB | 525 MB | 181.5 | 5 | 0.21 s | mem -1%, CPU -6% |
| brave | Blink | 538 MB | 546 MB | 32.4 | 12 | 0.33 s | mem +1%, CPU -2% |
| edge | Blink | 568 MB | 684 MB | 34.9 | 12 | 0.33 s | mem +1%, CPU +2% |
| floorp | Gecko | 726 MB | 897 MB | 44.6 | 10 | 0.93 s | mem +1%, CPU -2% |
| waterfox | Gecko | 733 MB | 1088 MB | 48.6 | 11 | 1.24 s | mem +0%, CPU -2% |
| vivaldi | Blink | 788 MB | 949 MB | 47.0 | 13 | 0.43 s | mem -1%, CPU -0% |
| firefox | Gecko | 841 MB | 934 MB | 47.4 | 11 | 0.83 s | mem +0%, CPU -4% |
| zen | Gecko | 842 MB | 1037 MB | 64.7 | 12 | 0.93 s | mem +1%, CPU -4% |
| librewolf | Gecko | 1167 MB | 1412 MB | 44.5 | 10 | 0.93 s | mem +0%, CPU -0% |
| chromium | Blink | failed: video not playing (page said cannot-play; windows: h264 1080 | | | | | |
| opera | Blink | failed: video not playing (page said cannot-play; windows: h264 1080 | | | | | |
| gnome-web | WebKit | failed: video not playing (page said cannot-play; windows: h264 1080 | | | | | |

### pan

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261126-0524-4c931f8-45761289882.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 749 MB | 753 MB | 361.9 | 14 | 0.23 s | mem +0%, CPU -2% |

## Trailboard over time (newest first)

| Run | Trailboard | idle mem / CPU | still mem / CPU | video-vp9 mem / CPU | video-av1 mem / CPU | video-h264 mem / CPU | pan mem / CPU |
|---|---|---|---|---|---|---|---|
| [20261126-0725-b](https://github.com/AyushKr2003/trailboard/actions/runs/45773585181) | `bd53a3d` | 578 MB / 4.7 % | 697 MB / 0.8 % | 619 MB / 366.2 % | 646 MB / 362.8 % | 646 MB / 364.3 % | 749 MB / 361.9 % |
| [20261126-0524-4](https://github.com/AyushKr2003/trailboard/actions/runs/45761289882) | `4c931f8` | 577 MB / 5.1 % | 699 MB / 0.6 % | 622 MB / 365.8 % | 672 MB / 376.2 % | 651 MB / 363.0 % | 746 MB / 368.2 % |
| [20261126-0418-e](https://github.com/AyushKr2003/trailboard/actions/runs/45754226156) | `ea5b59f` | 582 MB / 45.6 % | 697 MB / 0.5 % | 617 MB / 365.2 % | 648 MB / 372.2 % | 643 MB / 365.4 % | 738 MB / 358.0 % |
| [20261125-1734-e](https://github.com/AyushKr2003/trailboard/actions/runs/45665317928) | `ea5b59f` | 570 MB / 37.3 % | 684 MB / 0.6 % | 603 MB / 366.1 % | 640 MB / 364.1 % | 639 MB / 372.5 % | 742 MB / 359.7 % |
