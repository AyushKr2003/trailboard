# Results

Written by the bench workflow after every run. Chart: <https://ayushkr2003.github.io/trailboard/>

## Bench: Trailboard 4c931f8 against 16 browsers

20261126-0524 · runner 4 × Intel(R) Xeon(R) 6975P-C, 15 GB · 3 runs each, medians · [workflow run](https://github.com/AyushKr2003/trailboard/actions/runs/45761289882)

✅ No regression against the last run.

Failed to run: chromium, gnome-web, opera (see the logs).

### idle

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change since last run |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 577 MB | 578 MB | 5.1 | 9 | 0.21 s | mem -1%, CPU -89% |
| palemoon | Goanna | 263 MB | 284 MB | 0.6 | 1 | 0.52 s | mem +0%, CPU -14% |
| falkon | Blink (Qt WebEngine) | 398 MB | 412 MB | 10.1 | 5 | 0.21 s | mem -0%, CPU -18% |
| qutebrowser | Blink (Qt WebEngine) | 422 MB | 435 MB | 0.0 | 5 | 0.52 s | mem -0% |
| ungoogled-chromium | Blink | 429 MB | 442 MB | 0.7 | 10 | 0.22 s | mem +0%, CPU -13% |
| gnome-web | WebKit | 434 MB | 437 MB | 0.0 | 8 | 0.31 s | mem -9% |
| chrome | Blink | 438 MB | 496 MB | 0.8 | 13 | 0.33 s | mem +0%, CPU +0% |
| brave | Blink | 467 MB | 492 MB | 0.9 | 12 | 0.33 s | mem -0%, CPU -18% |
| edge | Blink | 495 MB | 607 MB | 0.6 | 12 | 0.33 s | mem +1%, CPU +0% |
| chromium | Blink | 499 MB | 540 MB | 0.9 | 12 | 0.32 s | mem -1%, CPU -10% |
| opera | Blink | 550 MB | 667 MB | 7.6 | 14 | 0.32 s | mem -1%, CPU -1% |
| waterfox | Gecko | 566 MB | 723 MB | 8.9 | 10 | 1.24 s | mem +0%, CPU -27% |
| floorp | Gecko | 646 MB | 820 MB | 12.1 | 9 | 0.93 s | mem +1%, CPU +256% |
| zen | Gecko | 668 MB | 836 MB | 16.6 | 11 | 0.93 s | mem -0%, CPU -32% |
| vivaldi | Blink | 690 MB | 859 MB | 6.4 | 12 | 0.43 s | mem +0%, CPU -2% |
| firefox | Gecko | 751 MB | 853 MB | 5.1 | 10 | 0.93 s | mem +1%, CPU -39% |
| librewolf | Gecko | 988 MB | 1275 MB | 2.9 | 9 | 0.93 s | mem +35%, CPU -37% |

### still

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change since last run |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 699 MB | 705 MB | 0.6 | 14 | 0.21 s | mem +0%, CPU +20% |
| palemoon | Goanna | 208 MB | 211 MB | 0.1 | 1 | 0.52 s | mem +0%, CPU +0% |
| falkon | Blink (Qt WebEngine) | 428 MB | 440 MB | 0.3 | 5 | 0.21 s | mem -1%, CPU +200% |
| ungoogled-chromium | Blink | 528 MB | 541 MB | 0.2 | 15 | 0.34 s | mem +0%, CPU +0% |
| chrome | Blink | 535 MB | 586 MB | 0.3 | 18 | 0.33 s | mem +0%, CPU -25% |
| brave | Blink | 560 MB | 577 MB | 0.1 | 17 | 0.33 s | mem -0%, CPU +0% |
| qutebrowser | Blink (Qt WebEngine) | 568 MB | 582 MB | 0.0 | 10 | 0.52 s | mem -0% |
| edge | Blink | 587 MB | 707 MB | 0.3 | 17 | 0.33 s | mem +1%, CPU +50% |
| chromium | Blink | 602 MB | 652 MB | 0.5 | 17 | 0.34 s | mem +0%, CPU +25% |
| floorp | Gecko | 663 MB | 857 MB | 2.4 | 15 | 0.83 s | mem -9%, CPU +0% |
| opera | Blink | 664 MB | 797 MB | 0.7 | 20 | 0.33 s | mem -0%, CPU +75% |
| waterfox | Gecko | 674 MB | 1060 MB | 9.2 | 16 | 1.24 s | mem +3%, CPU +24% |
| zen | Gecko | 756 MB | 912 MB | 16.8 | 17 | 0.84 s | mem -0%, CPU +11% |
| firefox | Gecko | 816 MB | 909 MB | 5.0 | 16 | 0.83 s | mem -0%, CPU +56% |
| vivaldi | Blink | 862 MB | 1008 MB | 1.4 | 18 | 0.44 s | mem +0%, CPU +27% |
| gnome-web | WebKit | 1022 MB | 1115 MB | 0.1 | 38 | 0.31 s | mem -2%, CPU +0% |
| librewolf | Gecko | 1096 MB | 1384 MB | 2.8 | 15 | 0.83 s | mem +1%, CPU -3% |

### video-vp9

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change since last run |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 622 MB | 622 MB | 365.8 | 9 | 0.21 s | mem +1%, CPU +0% |
| palemoon | Goanna | 261 MB | 267 MB | 30.1 | 1 | 0.52 s |  |
| qutebrowser | Blink (Qt WebEngine) | 460 MB | 478 MB | 197.6 | 5 | 0.52 s | mem -4%, CPU -1% |
| ungoogled-chromium | Blink | 489 MB | 500 MB | 41.7 | 10 | 0.22 s | mem -0%, CPU +4% |
| chrome | Blink | 497 MB | 556 MB | 39.3 | 12 | 0.22 s | mem +0%, CPU -1% |
| falkon | Blink (Qt WebEngine) | 497 MB | 497 MB | 191.1 | 5 | 0.21 s | mem +1%, CPU -1% |
| brave | Blink | 514 MB | 524 MB | 37.1 | 12 | 0.33 s | mem -0%, CPU -1% |
| edge | Blink | 544 MB | 656 MB | 39.1 | 12 | 0.32 s | mem +0%, CPU -4% |
| chromium | Blink | 558 MB | 602 MB | 37.5 | 12 | 0.22 s | mem +0%, CPU -6% |
| opera | Blink | 604 MB | 720 MB | 42.8 | 15 | 0.33 s | mem -0%, CPU +3% |
| gnome-web | WebKit | 647 MB | 663 MB | 0.0 | 8 | 0.31 s |  |
| waterfox | Gecko | 702 MB | 863 MB | 47.6 | 11 | 1.24 s | mem -1%, CPU -4% |
| floorp | Gecko | 714 MB | 879 MB | 43.7 | 10 | 0.93 s |  |
| vivaldi | Blink | 774 MB | 931 MB | 52.0 | 13 | 0.42 s | mem +1%, CPU +7% |
| zen | Gecko | 806 MB | 997 MB | 67.5 | 12 | 0.93 s | mem -1%, CPU +6% |
| firefox | Gecko | 829 MB | 918 MB | 48.6 | 11 | 0.83 s | mem +0%, CPU -2% |
| librewolf | Gecko | 1133 MB | 1327 MB | 45.4 | 10 | 0.93 s | mem -1%, CPU +2% |

### video-av1

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change since last run |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 672 MB | 674 MB | 376.2 | 9 | 0.24 s | mem +4%, CPU +1% |
| palemoon | Goanna | 337 MB | 346 MB | 40.2 | 1 | 0.65 s |  |
| chrome | Blink | 539 MB | 599 MB | 45.1 | 13 | 0.38 s | mem +2%, CPU +49% |
| ungoogled-chromium | Blink | 539 MB | 544 MB | 46.8 | 10 | 0.35 s | mem +2%, CPU +49% |
| brave | Blink | 562 MB | 565 MB | 42.8 | 12 | 0.36 s | mem +2%, CPU +49% |
| qutebrowser | Blink (Qt WebEngine) | 564 MB | 608 MB | 257.0 | 5 | 0.65 s | mem -7%, CPU +50% |
| edge | Blink | 589 MB | 696 MB | 45.7 | 12 | 0.48 s | mem +2%, CPU +50% |
| chromium | Blink | 606 MB | 644 MB | 44.4 | 12 | 0.36 s | mem +3%, CPU +47% |
| falkon | Blink (Qt WebEngine) | 608 MB | 639 MB | 235.9 | 5 | 0.35 s | mem +6%, CPU +44% |
| opera | Blink | 651 MB | 767 MB | 41.6 | 15 | 0.37 s | mem +2%, CPU +31% |
| waterfox | Gecko | 777 MB | 1131 MB | 58.1 | 11 | 1.69 s | mem +3%, CPU +21% |
| floorp | Gecko | 780 MB | 880 MB | 50.8 | 10 | 1.19 s |  |
| vivaldi | Blink | 821 MB | 978 MB | 54.8 | 13 | 0.59 s | mem +2%, CPU +43% |
| zen | Gecko | 886 MB | 1063 MB | 74.8 | 12 | 1.17 s | mem +3%, CPU +19% |
| firefox | Gecko | 891 MB | 965 MB | 57.1 | 11 | 1.28 s | mem +3%, CPU +16% |
| librewolf | Gecko | 1206 MB | 1503 MB | 51.6 | 10 | 1.27 s | mem -14%, CPU +21% |

### video-h264

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change since last run |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 651 MB | 652 MB | 363.0 | 9 | 0.21 s | mem +1%, CPU -1% |
| palemoon | Goanna | 273 MB | 282 MB | 32.0 | 1 | 0.52 s |  |
| qutebrowser | Blink (Qt WebEngine) | 488 MB | 496 MB | 197.5 | 5 | 0.52 s | mem +0%, CPU +5% |
| ungoogled-chromium | Blink | 511 MB | 520 MB | 35.0 | 10 | 0.22 s | mem -0%, CPU +5% |
| falkon | Blink (Qt WebEngine) | 520 MB | 523 MB | 192.4 | 5 | 0.21 s | mem +2%, CPU +9% |
| chrome | Blink | 525 MB | 578 MB | 34.3 | 13 | 0.23 s | mem +2%, CPU +6% |
| brave | Blink | 534 MB | 540 MB | 32.9 | 12 | 0.32 s | mem -1%, CPU +4% |
| edge | Blink | 563 MB | 676 MB | 34.2 | 12 | 0.33 s | mem -1%, CPU +2% |
| floorp | Gecko | 721 MB | 866 MB | 45.7 | 10 | 0.93 s |  |
| waterfox | Gecko | 732 MB | 1092 MB | 49.6 | 11 | 1.24 s | mem +0%, CPU +2% |
| vivaldi | Blink | 793 MB | 957 MB | 47.2 | 13 | 0.44 s | mem +0%, CPU +4% |
| zen | Gecko | 830 MB | 1041 MB | 67.5 | 12 | 0.93 s | mem -1%, CPU +6% |
| firefox | Gecko | 837 MB | 924 MB | 49.5 | 11 | 0.83 s | mem -1%, CPU +5% |
| librewolf | Gecko | 1163 MB | 1582 MB | 44.7 | 10 | 0.93 s | mem -1%, CPU +4% |
| chromium | Blink | failed: video not playing (page said cannot-play; windows: h264 1080 | | | | | |
| opera | Blink | failed: video not playing (page said cannot-play; windows: h264 1080 | | | | | |
| gnome-web | WebKit | failed: video not playing (page said cannot-play; windows: h264 1080 | | | | | |

### pan

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change since last run |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 746 MB | 750 MB | 368.2 | 14 | 0.23 s | mem +1%, CPU +3% |

## Trailboard over time (newest first)

| Run | Trailboard | idle mem / CPU | still mem / CPU | video-vp9 mem / CPU | video-av1 mem / CPU | video-h264 mem / CPU | pan mem / CPU |
|---|---|---|---|---|---|---|---|
| [20261126-0524-4](https://github.com/AyushKr2003/trailboard/actions/runs/45761289882) | `4c931f8` | 577 MB / 5.1 % | 699 MB / 0.6 % | 622 MB / 365.8 % | 672 MB / 376.2 % | 651 MB / 363.0 % | 746 MB / 368.2 % |
| [20261126-0418-e](https://github.com/AyushKr2003/trailboard/actions/runs/45754226156) | `ea5b59f` | 582 MB / 45.6 % | 697 MB / 0.5 % | 617 MB / 365.2 % | 648 MB / 372.2 % | 643 MB / 365.4 % | 738 MB / 358.0 % |
| [20261125-1734-e](https://github.com/AyushKr2003/trailboard/actions/runs/45665317928) | `ea5b59f` | 570 MB / 37.3 % | 684 MB / 0.6 % | 603 MB / 366.1 % | 640 MB / 364.1 % | 639 MB / 372.5 % | 742 MB / 359.7 % |
