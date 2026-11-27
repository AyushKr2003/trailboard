# Results

Written by the bench workflow after every run. Chart: <https://ayushkr2003.github.io/trailboard/>

## Bench: Trailboard 5aa154c against 16 browsers

20261127-1223 · 3 runs each, medians · each case on its own runner · [workflow run](https://github.com/AyushKr2003/trailboard/actions/runs/45998021086)

✅ No regression against earlier runs on the same CPU (not checked, no earlier run on their CPU: still).

Failed to run: vivaldi (see the logs).

Not run, by design (see browsers.toml): chromium (video-h264); opera (video-h264); gnome-web (video-vp9, video-av1, video-h264).

### idle

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261126-1507-e1edf21-45845659696.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 579 MB | 579 MB | 3.7 | 9 | 0.21 s | mem +1%, CPU -21% |
| palemoon | Goanna | 259 MB | 275 MB | 0.6 | 1 | 0.52 s | mem +1%, CPU +20% |
| falkon | Blink (Qt WebEngine) | 394 MB | 402 MB | 10.3 | 5 | 0.21 s | mem -2%, CPU +5% |
| qutebrowser | Blink (Qt WebEngine) | 414 MB | 427 MB | 0.0 | 5 | 0.52 s | mem -2% |
| ungoogled-chromium | Blink | 431 MB | 441 MB | 0.8 | 10 | 0.22 s | mem +0%, CPU +14% |
| gnome-web | WebKit | 435 MB | 437 MB | 0.0 | 8 | 0.31 s | mem +0% |
| chrome | Blink | 436 MB | 494 MB | 0.7 | 13 | 0.23 s | mem +0%, CPU +0% |
| brave | Blink | 469 MB | 482 MB | 0.8 | 12 | 0.32 s | mem +0%, CPU -11% |
| edge | Blink | 495 MB | 607 MB | 0.6 | 12 | 0.32 s | mem -0%, CPU +0% |
| chromium | Blink | 496 MB | 534 MB | 0.9 | 11 | 0.32 s | mem -1%, CPU +0% |
| opera | Blink | 553 MB | 666 MB | 7.1 | 14 | 0.32 s | mem -0%, CPU +9% |
| waterfox | Gecko | 563 MB | 975 MB | 8.9 | 10 | 1.24 s | mem +1%, CPU +3% |
| floorp | Gecko | 655 MB | 825 MB | 12.6 | 9 | 0.93 s | mem +1%, CPU +17% |
| zen | Gecko | 676 MB | 934 MB | 18.2 | 11 | 0.93 s | mem -0%, CPU +0% |
| vivaldi | Blink | 685 MB | 860 MB | 5.9 | 12 | 0.32 s | mem -0%, CPU +0% |
| firefox | Gecko | 744 MB | 862 MB | 3.3 | 10 | 0.93 s | mem +0%, CPU -33% |
| librewolf | Gecko | 963 MB | 1290 MB | 2.7 | 9 | 0.93 s | mem -0%, CPU -4% |

### still

Runner: 4 × AMD EPYC 9V74 80-Core Processor, 15 GB; no earlier run on this CPU to compare with.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 680 MB | 693 MB | 0.8 | 14 | 0.22 s |  |
| palemoon | Goanna | 203 MB | 205 MB | 0.1 | 1 | 0.73 s |  |
| falkon | Blink (Qt WebEngine) | 417 MB | 435 MB | 0.3 | 5 | 0.32 s |  |
| ungoogled-chromium | Blink | 510 MB | 522 MB | 0.3 | 15 | 0.34 s |  |
| chrome | Blink | 516 MB | 575 MB | 0.3 | 18 | 0.43 s |  |
| brave | Blink | 536 MB | 548 MB | 0.1 | 17 | 0.43 s |  |
| qutebrowser | Blink (Qt WebEngine) | 542 MB | 556 MB | 0.0 | 10 | 0.53 s |  |
| opera | Blink | 559 MB | 692 MB | 0.5 | 20 | 0.34 s |  |
| chromium | Blink | 577 MB | 626 MB | 0.5 | 16 | 0.36 s |  |
| edge | Blink | 578 MB | 698 MB | 0.2 | 17 | 0.45 s |  |
| floorp | Gecko | 664 MB | 829 MB | 8.3 | 15 | 1.07 s |  |
| waterfox | Gecko | 679 MB | 1057 MB | 9.8 | 16 | 1.60 s |  |
| zen | Gecko | 743 MB | 963 MB | 19.5 | 17 | 1.06 s |  |
| firefox | Gecko | 793 MB | 882 MB | 11.9 | 16 | 1.18 s |  |
| vivaldi | Blink | 845 MB | 998 MB | 1.3 | 18 | 0.61 s |  |
| gnome-web | WebKit | 967 MB | 990 MB | 0.0 | 38 | 0.43 s |  |
| librewolf | Gecko | 1117 MB | 1293 MB | 9.1 | 15 | 1.05 s |  |

### video-vp9

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261126-0725-bd53a3d-45773585181.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 623 MB | 625 MB | 269.9 | 9 | 0.21 s | mem +1%, CPU -26% |
| palemoon | Goanna | 258 MB | 262 MB | 31.8 | 1 | 0.52 s | mem -2%, CPU +0% |
| qutebrowser | Blink (Qt WebEngine) | 460 MB | 471 MB | 204.2 | 5 | 0.52 s | mem -1%, CPU +3% |
| ungoogled-chromium | Blink | 476 MB | 485 MB | 39.6 | 10 | 0.32 s | mem -3%, CPU -2% |
| falkon | Blink (Qt WebEngine) | 487 MB | 489 MB | 188.3 | 5 | 0.21 s | mem -2%, CPU +1% |
| chrome | Blink | 494 MB | 551 MB | 40.3 | 13 | 0.32 s | mem +0%, CPU -0% |
| brave | Blink | 514 MB | 520 MB | 36.1 | 12 | 0.32 s | mem -0%, CPU -4% |
| edge | Blink | 543 MB | 652 MB | 39.2 | 12 | 0.32 s | mem +0%, CPU -2% |
| chromium | Blink | 547 MB | 590 MB | 39.4 | 11 | 0.22 s | mem -1%, CPU -2% |
| opera | Blink | 607 MB | 731 MB | 40.2 | 15 | 0.34 s | mem +0%, CPU -4% |
| waterfox | Gecko | 702 MB | 832 MB | 49.5 | 11 | 1.35 s | mem -0%, CPU +2% |
| floorp | Gecko | 714 MB | 880 MB | 45.0 | 10 | 0.94 s | mem -0%, CPU +1% |
| vivaldi | Blink | 769 MB | 929 MB | 50.9 | 13 | 0.44 s | mem +0%, CPU -1% |
| zen | Gecko | 812 MB | 964 MB | 64.1 | 12 | 0.94 s | mem -0%, CPU -4% |
| firefox | Gecko | 822 MB | 897 MB | 47.9 | 11 | 0.93 s | mem -1%, CPU -1% |
| librewolf | Gecko | 1143 MB | 1386 MB | 45.4 | 10 | 0.94 s | mem -1%, CPU -4% |

### video-av1

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261126-1507-e1edf21-45845659696.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 629 MB | 633 MB | 248.1 | 9 | 0.21 s | mem -0%, CPU -32% |
| palemoon | Goanna | 282 MB | 287 MB | 28.3 | 1 | 0.52 s | mem +0%, CPU -14% |
| opera | Blink | 482 MB | 612 MB | 29.8 | 15 | 0.32 s | mem -0%, CPU -23% |
| ungoogled-chromium | Blink | 503 MB | 511 MB | 31.5 | 10 | 0.22 s | mem +0%, CPU -15% |
| chrome | Blink | 504 MB | 565 MB | 30.3 | 12 | 0.33 s | mem -0%, CPU -15% |
| brave | Blink | 528 MB | 533 MB | 28.9 | 12 | 0.32 s | mem +0%, CPU -13% |
| qutebrowser | Blink (Qt WebEngine) | 557 MB | 593 MB | 168.9 | 5 | 0.52 s | mem +0%, CPU -14% |
| edge | Blink | 559 MB | 674 MB | 30.6 | 12 | 0.32 s | mem +1%, CPU -14% |
| chromium | Blink | 564 MB | 601 MB | 31.0 | 11 | 0.32 s | mem -1%, CPU -13% |
| falkon | Blink (Qt WebEngine) | 605 MB | 632 MB | 157.3 | 5 | 0.21 s | mem +5%, CPU -14% |
| waterfox | Gecko | 733 MB | 860 MB | 47.8 | 11 | 1.24 s | mem -1%, CPU -5% |
| floorp | Gecko | 738 MB | 930 MB | 43.6 | 10 | 0.94 s | mem -1%, CPU -7% |
| vivaldi | Blink | 781 MB | 936 MB | 38.7 | 13 | 0.44 s | mem -0%, CPU -20% |
| zen | Gecko | 846 MB | 1046 MB | 61.5 | 12 | 0.94 s | mem -0%, CPU -8% |
| firefox | Gecko | 859 MB | 980 MB | 46.1 | 11 | 0.94 s | mem +0%, CPU -8% |
| librewolf | Gecko | 1173 MB | 1382 MB | 45.1 | 10 | 0.93 s | mem -16%, CPU -2% |

### video-h264

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261127-1121-879f0c2-45989683312.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 642 MB | 643 MB | 248.5 | 9 | 0.21 s | mem -0%, CPU -9% |
| palemoon | Goanna | 275 MB | 281 MB | 28.7 | 1 | 0.51 s | mem +0%, CPU -17% |
| qutebrowser | Blink (Qt WebEngine) | 482 MB | 490 MB | 172.7 | 5 | 0.52 s | mem -3%, CPU -15% |
| falkon | Blink (Qt WebEngine) | 506 MB | 513 MB | 167.6 | 5 | 0.21 s | mem -3%, CPU -16% |
| ungoogled-chromium | Blink | 512 MB | 521 MB | 31.6 | 10 | 0.32 s | mem +0%, CPU -17% |
| chrome | Blink | 518 MB | 580 MB | 30.5 | 12 | 0.22 s | mem +0%, CPU -17% |
| brave | Blink | 537 MB | 544 MB | 29.4 | 12 | 0.32 s | mem -0%, CPU -15% |
| edge | Blink | 567 MB | 680 MB | 30.6 | 12 | 0.32 s | mem +0%, CPU -18% |
| floorp | Gecko | 728 MB | 922 MB | 44.0 | 10 | 0.83 s | mem +1%, CPU -6% |
| waterfox | Gecko | 731 MB | 1090 MB | 48.0 | 11 | 1.14 s | mem -1%, CPU -6% |
| zen | Gecko | 822 MB | 998 MB | 64.7 | 12 | 0.82 s | mem -1%, CPU -9% |
| firefox | Gecko | 840 MB | 922 MB | 46.4 | 11 | 0.82 s | mem -1%, CPU -5% |
| librewolf | Gecko | 1178 MB | 1422 MB | 44.8 | 10 | 0.82 s | mem +26%, CPU -5% |
| vivaldi | Blink | failed: exited with -5 and no window | | | | | |

### pan

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261127-1121-879f0c2-45989683312.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 801 MB | 804 MB | 350.2 | 14 | 0.22 s | mem -1%, CPU -1% |

## Trailboard over time (newest first)

| Run | Trailboard | idle mem / CPU | still mem / CPU | video-vp9 mem / CPU | video-av1 mem / CPU | video-h264 mem / CPU | pan mem / CPU |
|---|---|---|---|---|---|---|---|
| [20261127-1223-5](https://github.com/AyushKr2003/trailboard/actions/runs/45998021086) | `5aa154c` | 579 MB / 3.7 % | 680 MB / 0.8 % | 623 MB / 269.9 % | 629 MB / 248.1 % | 642 MB / 248.5 % | 801 MB / 350.2 % |
| [20261127-1121-8](https://github.com/AyushKr2003/trailboard/actions/runs/45989683312) | `879f0c2` | 585 MB / 3.2 % | 704 MB / 0.7 % | 611 MB / 347.8 % | 646 MB / 332.3 % | 645 MB / 272.2 % | 812 MB / 352.6 % |
| [20261126-1507-e](https://github.com/AyushKr2003/trailboard/actions/runs/45845659696) | `e1edf21` | 574 MB / 4.7 % | 700 MB / 0.9 % | 591 MB / 366.7 % | 632 MB / 363.4 % | 631 MB / 358.9 % | 792 MB / 375.2 % |
| [20261126-0725-b](https://github.com/AyushKr2003/trailboard/actions/runs/45773585181) | `bd53a3d` | 578 MB / 4.7 % | 697 MB / 0.8 % | 619 MB / 366.2 % | 646 MB / 362.8 % | 646 MB / 364.3 % | 749 MB / 361.9 % |
| [20261126-0524-4](https://github.com/AyushKr2003/trailboard/actions/runs/45761289882) | `4c931f8` | 577 MB / 5.1 % | 699 MB / 0.6 % | 622 MB / 365.8 % | 672 MB / 376.2 % | 651 MB / 363.0 % | 746 MB / 368.2 % |
| [20261126-0418-e](https://github.com/AyushKr2003/trailboard/actions/runs/45754226156) | `ea5b59f` | 582 MB / 45.6 % | 697 MB / 0.5 % | 617 MB / 365.2 % | 648 MB / 372.2 % | 643 MB / 365.4 % | 738 MB / 358.0 % |
| [20261125-1734-e](https://github.com/AyushKr2003/trailboard/actions/runs/45665317928) | `ea5b59f` | 570 MB / 37.3 % | 684 MB / 0.6 % | 603 MB / 366.1 % | 640 MB / 364.1 % | 639 MB / 372.5 % | 742 MB / 359.7 % |
