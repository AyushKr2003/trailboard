# Results

Written by the bench workflow after every run. Chart: <https://ayushkr2003.github.io/trailboard/>

## Bench: Trailboard 879f0c2 against 16 browsers

20261127-1121 · 3 runs each, medians · each case on its own runner · [workflow run](https://github.com/AyushKr2003/trailboard/actions/runs/45989683312)

✅ No regression against earlier runs on the same CPU (not checked, no earlier run on their CPU: idle, video-vp9).

Not run, by design (see browsers.toml): chromium (video-h264); opera (video-h264); gnome-web (video-vp9, video-av1, video-h264).

### idle

Runner: 4 × INTEL(R) XEON(R) PLATINUM 8573C, 15 GB; no earlier run on this CPU to compare with.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 585 MB | 586 MB | 3.2 | 9 | 0.32 s |  |
| palemoon | Goanna | 262 MB | 284 MB | 0.6 | 1 | 0.63 s |  |
| falkon | Blink (Qt WebEngine) | 395 MB | 402 MB | 12.3 | 5 | 0.32 s |  |
| qutebrowser | Blink (Qt WebEngine) | 419 MB | 427 MB | 0.0 | 5 | 0.62 s |  |
| ungoogled-chromium | Blink | 429 MB | 440 MB | 0.8 | 10 | 0.33 s |  |
| gnome-web | WebKit | 433 MB | 436 MB | 0.0 | 8 | 0.42 s |  |
| chrome | Blink | 437 MB | 498 MB | 0.8 | 13 | 0.32 s |  |
| brave | Blink | 470 MB | 478 MB | 1.0 | 12 | 0.33 s |  |
| chromium | Blink | 493 MB | 535 MB | 1.1 | 11 | 0.33 s |  |
| edge | Blink | 500 MB | 613 MB | 0.6 | 12 | 0.44 s |  |
| opera | Blink | 552 MB | 657 MB | 8.3 | 14 | 0.43 s |  |
| waterfox | Gecko | 567 MB | 977 MB | 11.6 | 10 | 1.66 s |  |
| zen | Gecko | 667 MB | 885 MB | 23.3 | 11 | 1.24 s |  |
| floorp | Gecko | 681 MB | 827 MB | 15.4 | 9 | 1.15 s |  |
| vivaldi | Blink | 690 MB | 858 MB | 6.5 | 12 | 0.55 s |  |
| firefox | Gecko | 742 MB | 828 MB | 8.1 | 10 | 1.14 s |  |
| librewolf | Gecko | 1067 MB | 1365 MB | 3.4 | 9 | 1.25 s |  |

### still

Runner: 4 × AMD EPYC 7763 64-Core Processor, 15 GB; compared with 20261125-1734-ea5b59f-45665317928.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 704 MB | 712 MB | 0.7 | 14 | 0.32 s | mem +3%, CPU +17% |
| palemoon | Goanna | 205 MB | 208 MB | 0.1 | 1 | 0.74 s | mem +133% |
| falkon | Blink (Qt WebEngine) | 429 MB | 448 MB | 0.3 | 5 | 0.32 s | mem -1%, CPU +50% |
| ungoogled-chromium | Blink | 526 MB | 537 MB | 0.3 | 15 | 0.45 s |  |
| chrome | Blink | 532 MB | 587 MB | 0.4 | 18 | 0.46 s | mem -3%, CPU +0% |
| qutebrowser | Blink (Qt WebEngine) | 558 MB | 572 MB | 0.0 | 10 | 0.74 s | mem -6% |
| brave | Blink | 560 MB | 579 MB | 0.1 | 17 | 0.45 s | mem -1%, CPU -50% |
| edge | Blink | 593 MB | 714 MB | 0.4 | 17 | 0.56 s | mem +0%, CPU +33% |
| chromium | Blink | 595 MB | 642 MB | 0.7 | 16 | 0.58 s | mem -2%, CPU +17% |
| floorp | Gecko | 661 MB | 842 MB | 3.4 | 15 | 1.28 s |  |
| waterfox | Gecko | 667 MB | 1055 MB | 13.4 | 16 | 1.79 s |  |
| opera | Blink | 671 MB | 784 MB | 0.6 | 20 | 0.46 s | mem +0%, CPU -14% |
| zen | Gecko | 748 MB | 1004 MB | 25.2 | 17 | 1.29 s | mem +373% |
| firefox | Gecko | 805 MB | 866 MB | 5.1 | 16 | 1.38 s | mem +444% |
| vivaldi | Blink | 859 MB | 1036 MB | 1.6 | 18 | 0.68 s | mem -1%, CPU +7% |
| gnome-web | WebKit | 940 MB | 1127 MB | 0.2 | 38 | 0.53 s | mem -19%, CPU +100% |
| librewolf | Gecko | 1167 MB | 1346 MB | 3.9 | 15 | 1.39 s | mem +616% |

### video-vp9

Runner: 4 × AMD EPYC 7763 64-Core Processor, 15 GB; no earlier run on this CPU to compare with.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 611 MB | 613 MB | 347.8 | 9 | 0.33 s |  |
| palemoon | Goanna | 256 MB | 262 MB | 37.5 | 1 | 0.75 s |  |
| qutebrowser | Blink (Qt WebEngine) | 447 MB | 457 MB | 267.2 | 5 | 0.74 s |  |
| ungoogled-chromium | Blink | 472 MB | 484 MB | 52.4 | 10 | 0.45 s |  |
| falkon | Blink (Qt WebEngine) | 480 MB | 480 MB | 245.8 | 5 | 0.33 s |  |
| chrome | Blink | 483 MB | 541 MB | 51.0 | 13 | 0.45 s |  |
| opera | Blink | 488 MB | 610 MB | 46.4 | 15 | 0.44 s |  |
| brave | Blink | 502 MB | 510 MB | 47.6 | 12 | 0.44 s |  |
| edge | Blink | 531 MB | 646 MB | 51.2 | 12 | 0.48 s |  |
| chromium | Blink | 540 MB | 582 MB | 52.3 | 11 | 0.37 s |  |
| waterfox | Gecko | 693 MB | 1050 MB | 56.6 | 11 | 1.82 s |  |
| floorp | Gecko | 706 MB | 803 MB | 48.4 | 10 | 1.40 s |  |
| vivaldi | Blink | 756 MB | 909 MB | 60.2 | 13 | 0.69 s |  |
| zen | Gecko | 797 MB | 978 MB | 76.5 | 12 | 1.42 s |  |
| firefox | Gecko | 809 MB | 885 MB | 57.2 | 11 | 1.50 s |  |
| librewolf | Gecko | 1116 MB | 1316 MB | 53.0 | 10 | 1.61 s |  |

### video-av1

Runner: 4 × INTEL(R) XEON(R) PLATINUM 8573C, 15 GB; compared with 20261126-0725-bd53a3d-45773585181.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 646 MB | 649 MB | 332.3 | 9 | 0.32 s | mem +0%, CPU -8% |
| palemoon | Goanna | 284 MB | 287 MB | 47.6 | 1 | 0.74 s | mem -2%, CPU +9% |
| ungoogled-chromium | Blink | 502 MB | 508 MB | 49.6 | 10 | 0.46 s | mem -4%, CPU +5% |
| chrome | Blink | 524 MB | 572 MB | 47.9 | 13 | 0.43 s | mem -1%, CPU +3% |
| opera | Blink | 527 MB | 634 MB | 48.5 | 15 | 0.44 s | mem -17%, CPU +4% |
| brave | Blink | 530 MB | 532 MB | 46.2 | 12 | 0.43 s | mem -3%, CPU +5% |
| qutebrowser | Blink (Qt WebEngine) | 531 MB | 569 MB | 267.4 | 5 | 0.74 s | mem -6%, CPU +6% |
| chromium | Blink | 565 MB | 603 MB | 48.9 | 11 | 0.35 s | mem -5%, CPU +4% |
| falkon | Blink (Qt WebEngine) | 567 MB | 588 MB | 260.2 | 5 | 0.32 s | mem -8%, CPU +11% |
| edge | Blink | 568 MB | 679 MB | 48.6 | 12 | 0.44 s | mem -1%, CPU +5% |
| waterfox | Gecko | 736 MB | 869 MB | 67.9 | 11 | 1.90 s | mem -1%, CPU +4% |
| floorp | Gecko | 742 MB | 837 MB | 61.4 | 10 | 1.38 s | mem -1%, CPU +5% |
| vivaldi | Blink | 785 MB | 946 MB | 62.8 | 13 | 0.58 s | mem -3%, CPU +5% |
| zen | Gecko | 845 MB | 1024 MB | 89.2 | 12 | 1.38 s | mem -0%, CPU +3% |
| firefox | Gecko | 855 MB | 935 MB | 67.8 | 11 | 1.48 s | mem -2%, CPU +7% |
| librewolf | Gecko | 1146 MB | 1478 MB | 62.2 | 10 | 1.39 s | mem -22%, CPU +4% |

### video-h264

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261126-0725-bd53a3d-45773585181.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 645 MB | 646 MB | 272.2 | 9 | 0.21 s | mem -0%, CPU -25% |
| palemoon | Goanna | 274 MB | 282 MB | 34.6 | 1 | 0.52 s | mem +0%, CPU +13% |
| qutebrowser | Blink (Qt WebEngine) | 496 MB | 496 MB | 204.3 | 5 | 0.52 s | mem +2%, CPU +12% |
| ungoogled-chromium | Blink | 512 MB | 520 MB | 38.2 | 10 | 0.32 s | mem +0%, CPU +9% |
| chrome | Blink | 517 MB | 574 MB | 36.7 | 12 | 0.33 s | mem +0%, CPU +9% |
| falkon | Blink (Qt WebEngine) | 522 MB | 522 MB | 200.5 | 5 | 0.21 s | mem +1%, CPU +10% |
| brave | Blink | 539 MB | 543 MB | 34.6 | 12 | 0.33 s | mem +0%, CPU +7% |
| edge | Blink | 567 MB | 679 MB | 37.2 | 12 | 0.33 s | mem -0%, CPU +7% |
| floorp | Gecko | 724 MB | 892 MB | 46.6 | 10 | 0.93 s | mem -0%, CPU +4% |
| waterfox | Gecko | 735 MB | 1079 MB | 51.2 | 11 | 1.26 s | mem +0%, CPU +5% |
| vivaldi | Blink | 789 MB | 951 MB | 51.4 | 13 | 0.43 s | mem +0%, CPU +9% |
| zen | Gecko | 833 MB | 1044 MB | 70.9 | 12 | 0.93 s | mem -1%, CPU +10% |
| firefox | Gecko | 848 MB | 922 MB | 48.9 | 11 | 0.93 s | mem +1%, CPU +3% |
| librewolf | Gecko | 934 MB | 1383 MB | 47.0 | 10 | 0.93 s | mem -20%, CPU +6% |

### pan

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261126-0725-bd53a3d-45773585181.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 812 MB | 820 MB | 352.6 | 14 | 0.22 s | mem +8%, CPU -3% |

## Trailboard over time (newest first)

| Run | Trailboard | idle mem / CPU | still mem / CPU | video-vp9 mem / CPU | video-av1 mem / CPU | video-h264 mem / CPU | pan mem / CPU |
|---|---|---|---|---|---|---|---|
| [20261127-1121-8](https://github.com/AyushKr2003/trailboard/actions/runs/45989683312) | `879f0c2` | 585 MB / 3.2 % | 704 MB / 0.7 % | 611 MB / 347.8 % | 646 MB / 332.3 % | 645 MB / 272.2 % | 812 MB / 352.6 % |
| [20261126-1507-e](https://github.com/AyushKr2003/trailboard/actions/runs/45845659696) | `e1edf21` | 574 MB / 4.7 % | 700 MB / 0.9 % | 591 MB / 366.7 % | 632 MB / 363.4 % | 631 MB / 358.9 % | 792 MB / 375.2 % |
| [20261126-0725-b](https://github.com/AyushKr2003/trailboard/actions/runs/45773585181) | `bd53a3d` | 578 MB / 4.7 % | 697 MB / 0.8 % | 619 MB / 366.2 % | 646 MB / 362.8 % | 646 MB / 364.3 % | 749 MB / 361.9 % |
| [20261126-0524-4](https://github.com/AyushKr2003/trailboard/actions/runs/45761289882) | `4c931f8` | 577 MB / 5.1 % | 699 MB / 0.6 % | 622 MB / 365.8 % | 672 MB / 376.2 % | 651 MB / 363.0 % | 746 MB / 368.2 % |
| [20261126-0418-e](https://github.com/AyushKr2003/trailboard/actions/runs/45754226156) | `ea5b59f` | 582 MB / 45.6 % | 697 MB / 0.5 % | 617 MB / 365.2 % | 648 MB / 372.2 % | 643 MB / 365.4 % | 738 MB / 358.0 % |
| [20261125-1734-e](https://github.com/AyushKr2003/trailboard/actions/runs/45665317928) | `ea5b59f` | 570 MB / 37.3 % | 684 MB / 0.6 % | 603 MB / 366.1 % | 640 MB / 364.1 % | 639 MB / 372.5 % | 742 MB / 359.7 % |
