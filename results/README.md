# Results

Written by the bench workflow after every run. Chart: <https://ayushkr2003.github.io/trailboard/>

## Bench: Trailboard ea5b59f against 16 browsers

20261126-0418 · runner 4 × AMD EPYC 7763 64-Core Processor, 15 GB · 3 runs each, medians · [workflow run](https://github.com/AyushKr2003/trailboard/actions/runs/45754226156)

✅ No regression against the last run.

Failed to run: chromium, floorp, gnome-web, opera, palemoon (see the logs).

### idle

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change since last run |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 582 MB | 582 MB | 45.6 | 9 | 0.32 s | mem +2%, CPU +22% |
| palemoon | Goanna | 262 MB | 280 MB | 0.7 | 1 | 0.64 s | mem +198% |
| falkon | Blink (Qt WebEngine) | 400 MB | 401 MB | 12.3 | 5 | 0.32 s | mem +0%, CPU +1% |
| qutebrowser | Blink (Qt WebEngine) | 424 MB | 437 MB | 0.0 | 5 | 0.73 s | mem -6% |
| ungoogled-chromium | Blink | 429 MB | 439 MB | 0.8 | 10 | 0.33 s |  |
| chrome | Blink | 437 MB | 495 MB | 0.8 | 13 | 0.33 s | mem -2%, CPU +0% |
| brave | Blink | 469 MB | 478 MB | 1.1 | 12 | 0.44 s | mem -2%, CPU -15% |
| gnome-web | WebKit | 476 MB | 478 MB | 0.0 | 8 | 0.42 s | mem -20% |
| edge | Blink | 491 MB | 602 MB | 0.6 | 12 | 0.44 s | mem -2%, CPU -25% |
| chromium | Blink | 505 MB | 544 MB | 1.0 | 12 | 0.35 s | mem -0%, CPU -9% |
| opera | Blink | 553 MB | 653 MB | 7.7 | 14 | 0.44 s | mem -1%, CPU -21% |
| waterfox | Gecko | 566 MB | 975 MB | 12.2 | 10 | 1.68 s |  |
| floorp | Gecko | 637 MB | 803 MB | 3.4 | 10 | 1.17 s |  |
| zen | Gecko | 671 MB | 873 MB | 24.3 | 11 | 1.16 s | mem +322% |
| vivaldi | Blink | 689 MB | 849 MB | 6.5 | 12 | 0.58 s | mem -1%, CPU -23% |
| librewolf | Gecko | 730 MB | 1425 MB | 4.6 | 9 | 1.27 s | mem +348% |
| firefox | Gecko | 745 MB | 835 MB | 8.4 | 10 | 1.26 s | mem +407% |

### still

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change since last run |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 697 MB | 716 MB | 0.5 | 14 | 0.21 s | mem +2%, CPU -17% |
| palemoon | Goanna | 207 MB | 210 MB | 0.1 | 1 | 0.42 s | mem +135% |
| falkon | Blink (Qt WebEngine) | 434 MB | 447 MB | 0.1 | 5 | 0.21 s | mem +0%, CPU -50% |
| ungoogled-chromium | Blink | 528 MB | 539 MB | 0.2 | 15 | 0.35 s |  |
| chrome | Blink | 533 MB | 585 MB | 0.4 | 18 | 0.33 s | mem -2%, CPU +0% |
| brave | Blink | 561 MB | 577 MB | 0.1 | 17 | 0.33 s | mem -1%, CPU -50% |
| qutebrowser | Blink (Qt WebEngine) | 569 MB | 581 MB | 0.0 | 10 | 0.52 s | mem -5% |
| edge | Blink | 584 MB | 703 MB | 0.2 | 17 | 0.33 s | mem -1%, CPU -33% |
| chromium | Blink | 602 MB | 649 MB | 0.4 | 17 | 0.34 s | mem -1%, CPU -33% |
| waterfox | Gecko | 654 MB | 1056 MB | 7.4 | 16 | 1.18 s |  |
| opera | Blink | 665 MB | 797 MB | 0.4 | 20 | 0.34 s | mem -1%, CPU -43% |
| floorp | Gecko | 731 MB | 957 MB | 2.4 | 16 | 0.85 s |  |
| zen | Gecko | 758 MB | 957 MB | 15.2 | 17 | 0.85 s | mem +380% |
| firefox | Gecko | 820 MB | 900 MB | 3.2 | 16 | 0.85 s | mem +454% |
| vivaldi | Blink | 860 MB | 1012 MB | 1.1 | 18 | 0.44 s | mem -0%, CPU -27% |
| gnome-web | WebKit | 1043 MB | 1112 MB | 0.1 | 38 | 0.32 s | mem -10%, CPU +0% |
| librewolf | Gecko | 1086 MB | 1407 MB | 2.9 | 15 | 0.84 s | mem +566% |

### video-vp9

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change since last run |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 617 MB | 620 MB | 365.2 | 9 | 0.21 s | mem +2%, CPU -0% |
| qutebrowser | Blink (Qt WebEngine) | 479 MB | 481 MB | 199.6 | 5 | 0.52 s | mem -2%, CPU +1% |
| ungoogled-chromium | Blink | 491 MB | 504 MB | 40.1 | 10 | 0.22 s |  |
| falkon | Blink (Qt WebEngine) | 491 MB | 494 MB | 192.2 | 5 | 0.31 s | mem -2%, CPU +3% |
| chrome | Blink | 496 MB | 558 MB | 39.7 | 12 | 0.34 s | mem -1%, CPU -1% |
| brave | Blink | 515 MB | 524 MB | 37.3 | 12 | 0.32 s | mem -1%, CPU -3% |
| edge | Blink | 543 MB | 655 MB | 40.6 | 12 | 0.33 s | mem -1%, CPU +0% |
| chromium | Blink | 557 MB | 597 MB | 40.0 | 12 | 0.33 s | mem -1%, CPU -1% |
| opera | Blink | 606 MB | 726 MB | 41.6 | 15 | 0.33 s | mem -1%, CPU -4% |
| waterfox | Gecko | 710 MB | 1066 MB | 49.4 | 11 | 1.23 s |  |
| vivaldi | Blink | 770 MB | 930 MB | 48.7 | 13 | 0.44 s | mem -0%, CPU -9% |
| zen | Gecko | 812 MB | 1018 MB | 63.9 | 12 | 0.83 s | mem +408%, CPU +63800% |
| firefox | Gecko | 826 MB | 921 MB | 49.4 | 11 | 0.83 s | mem +466% |
| librewolf | Gecko | 1146 MB | 1341 MB | 44.5 | 10 | 0.93 s | mem +607% |
| floorp | Gecko | failed: video not playing (windows: Welcome to Floorp — Ablaze Floor | | | | | |
| palemoon | Goanna | failed: video not playing (windows: Pale Moon) | | | | | |
| gnome-web | WebKit | failed: video not playing (windows: Set as Default Browser?; vp9 108 | | | | | |

### video-av1

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change since last run |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 648 MB | 649 MB | 372.2 | 9 | 0.21 s | mem +1%, CPU +2% |
| ungoogled-chromium | Blink | 526 MB | 534 MB | 31.4 | 10 | 0.22 s |  |
| chrome | Blink | 530 MB | 586 MB | 30.3 | 12 | 0.22 s | mem -1%, CPU -17% |
| brave | Blink | 550 MB | 552 MB | 28.7 | 12 | 0.33 s | mem -1%, CPU -13% |
| falkon | Blink (Qt WebEngine) | 575 MB | 621 MB | 164.1 | 5 | 0.21 s | mem +0%, CPU -11% |
| edge | Blink | 577 MB | 687 MB | 30.5 | 12 | 0.33 s | mem -2%, CPU -14% |
| chromium | Blink | 591 MB | 632 MB | 30.3 | 12 | 0.22 s | mem -2%, CPU -16% |
| qutebrowser | Blink (Qt WebEngine) | 604 MB | 640 MB | 171.0 | 5 | 0.51 s | mem +1%, CPU -14% |
| opera | Blink | 636 MB | 752 MB | 31.8 | 15 | 0.33 s | mem -2%, CPU -21% |
| waterfox | Gecko | 751 MB | 883 MB | 48.2 | 11 | 1.23 s |  |
| vivaldi | Blink | 807 MB | 963 MB | 38.4 | 13 | 0.43 s | mem -1%, CPU -24% |
| zen | Gecko | 860 MB | 1034 MB | 62.7 | 12 | 0.83 s | mem +441% |
| firefox | Gecko | 863 MB | 957 MB | 49.1 | 11 | 0.82 s | mem +487% |
| librewolf | Gecko | 1397 MB | 1510 MB | 42.5 | 10 | 0.92 s | mem +757% |
| floorp | Gecko | failed: video not playing (windows: Welcome to Floorp — Ablaze Floor | | | | | |
| palemoon | Goanna | failed: video not playing (windows: Pale Moon) | | | | | |
| gnome-web | WebKit | failed: video not playing (windows: Set as Default Browser?; av1 108 | | | | | |

### video-h264

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change since last run |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 643 MB | 646 MB | 365.4 | 9 | 0.21 s | mem +1%, CPU -2% |
| qutebrowser | Blink (Qt WebEngine) | 487 MB | 489 MB | 187.8 | 5 | 0.52 s | mem -7%, CPU -30% |
| falkon | Blink (Qt WebEngine) | 511 MB | 512 MB | 176.2 | 5 | 0.31 s | mem -0%, CPU -31% |
| ungoogled-chromium | Blink | 513 MB | 522 MB | 33.4 | 10 | 0.21 s |  |
| chrome | Blink | 516 MB | 573 MB | 32.3 | 12 | 0.22 s | mem -2%, CPU -30% |
| brave | Blink | 538 MB | 544 MB | 31.6 | 12 | 0.33 s | mem -1%, CPU -28% |
| edge | Blink | 566 MB | 680 MB | 33.5 | 12 | 0.33 s | mem +0%, CPU -29% |
| waterfox | Gecko | 730 MB | 1086 MB | 48.4 | 11 | 1.24 s |  |
| vivaldi | Blink | 790 MB | 954 MB | 45.6 | 13 | 0.45 s | mem -2%, CPU -18% |
| zen | Gecko | 842 MB | 1018 MB | 63.9 | 12 | 0.83 s | mem +436% |
| firefox | Gecko | 845 MB | 936 MB | 47.2 | 11 | 0.83 s | mem +471% |
| librewolf | Gecko | 1174 MB | 1350 MB | 43.1 | 10 | 0.83 s | mem +620% |
| chromium | Blink | failed: video not playing (windows: h264 1080p: cannot play - Chromi | | | | | |
| opera | Blink | failed: video not playing (windows: h264 1080p: cannot play - Opera) | | | | | |
| floorp | Gecko | failed: video not playing (windows: Welcome to Floorp — Ablaze Floor | | | | | |
| palemoon | Goanna | failed: video not playing (windows: Pale Moon) | | | | | |
| gnome-web | WebKit | failed: video not playing (windows: Set as Default Browser?; h264 10 | | | | | |

### pan

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change since last run |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 738 MB | 745 MB | 358.0 | 14 | 0.36 s | mem -1%, CPU -0% |

## Trailboard over time (newest first)

| Run | Trailboard | idle mem / CPU | still mem / CPU | video-vp9 mem / CPU | video-av1 mem / CPU | video-h264 mem / CPU | pan mem / CPU |
|---|---|---|---|---|---|---|---|
| [20261126-0418-e](https://github.com/AyushKr2003/trailboard/actions/runs/45754226156) | `ea5b59f` | 582 MB / 45.6 % | 697 MB / 0.5 % | 617 MB / 365.2 % | 648 MB / 372.2 % | 643 MB / 365.4 % | 738 MB / 358.0 % |
| [20261125-1734-e](https://github.com/AyushKr2003/trailboard/actions/runs/45665317928) | `ea5b59f` | 570 MB / 37.3 % | 684 MB / 0.6 % | 603 MB / 366.1 % | 640 MB / 364.1 % | 639 MB / 372.5 % | 742 MB / 359.7 % |
