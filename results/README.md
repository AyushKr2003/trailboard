# Results

Written by the bench workflow after every run. Chart: <https://ayushkr2003.github.io/trailboard/>

## Bench: Trailboard e1edf21 against 16 browsers

20261126-1507 · 3 runs each, medians · each case on its own runner · [workflow run](https://github.com/AyushKr2003/trailboard/actions/runs/45845659696)

✅ No regression against earlier runs on the same CPU (not checked, no earlier run on their CPU: video-vp9, video-h264, pan).

Not run, by design (see browsers.toml): chromium (video-h264); opera (video-h264); gnome-web (video-vp9, video-av1, video-h264).

### idle

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261126-0725-bd53a3d-45773585181.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 574 MB | 576 MB | 4.7 | 9 | 0.21 s | mem -1%, CPU +0% |
| palemoon | Goanna | 256 MB | 266 MB | 0.5 | 1 | 0.52 s | mem -0%, CPU +0% |
| falkon | Blink (Qt WebEngine) | 403 MB | 411 MB | 9.8 | 5 | 0.21 s | mem +3%, CPU +0% |
| qutebrowser | Blink (Qt WebEngine) | 422 MB | 435 MB | 0.0 | 5 | 0.52 s | mem -2% |
| ungoogled-chromium | Blink | 431 MB | 443 MB | 0.7 | 10 | 0.21 s | mem +1%, CPU +0% |
| chrome | Blink | 435 MB | 493 MB | 0.7 | 12 | 0.32 s | mem +0%, CPU +17% |
| gnome-web | WebKit | 435 MB | 438 MB | 0.0 | 8 | 0.32 s | mem -0% |
| brave | Blink | 468 MB | 481 MB | 0.9 | 12 | 0.32 s | mem +0%, CPU +29% |
| edge | Blink | 496 MB | 608 MB | 0.6 | 12 | 0.33 s | mem +0%, CPU +20% |
| chromium | Blink | 499 MB | 541 MB | 0.9 | 12 | 0.22 s | mem -1%, CPU +12% |
| opera | Blink | 554 MB | 659 MB | 6.5 | 14 | 0.33 s | mem +1%, CPU +5% |
| waterfox | Gecko | 559 MB | 982 MB | 8.6 | 10 | 1.23 s | mem -2%, CPU -1% |
| floorp | Gecko | 646 MB | 789 MB | 10.8 | 9 | 0.93 s | mem -1%, CPU +2% |
| zen | Gecko | 679 MB | 922 MB | 18.2 | 11 | 0.83 s | mem +2%, CPU +11% |
| vivaldi | Blink | 688 MB | 860 MB | 5.9 | 12 | 0.42 s | mem +0%, CPU +18% |
| firefox | Gecko | 743 MB | 834 MB | 4.9 | 10 | 0.82 s | mem -1%, CPU +48% |
| librewolf | Gecko | 967 MB | 1229 MB | 2.8 | 9 | 0.93 s | mem -3%, CPU +8% |

### still

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261126-0725-bd53a3d-45773585181.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 700 MB | 714 MB | 0.9 | 14 | 0.21 s | mem +0%, CPU +12% |
| palemoon | Goanna | 209 MB | 212 MB | 0.1 | 1 | 0.52 s | mem +1%, CPU +0% |
| falkon | Blink (Qt WebEngine) | 427 MB | 444 MB | 0.3 | 5 | 0.21 s | mem -2%, CPU +0% |
| ungoogled-chromium | Blink | 527 MB | 539 MB | 0.3 | 15 | 0.33 s | mem +0%, CPU +0% |
| chrome | Blink | 529 MB | 587 MB | 0.3 | 18 | 0.33 s | mem +0%, CPU -25% |
| brave | Blink | 560 MB | 579 MB | 0.2 | 17 | 0.33 s | mem +6%, CPU +100% |
| qutebrowser | Blink (Qt WebEngine) | 570 MB | 585 MB | 0.0 | 10 | 0.52 s | mem +2% |
| edge | Blink | 588 MB | 709 MB | 0.2 | 17 | 0.32 s | mem +0%, CPU +0% |
| chromium | Blink | 596 MB | 651 MB | 0.6 | 17 | 0.32 s | mem -0%, CPU +20% |
| floorp | Gecko | 663 MB | 895 MB | 2.6 | 15 | 0.94 s | mem -0%, CPU -4% |
| opera | Blink | 666 MB | 802 MB | 0.6 | 20 | 0.33 s | mem +1%, CPU +0% |
| waterfox | Gecko | 681 MB | 1067 MB | 9.1 | 16 | 1.25 s | mem +1%, CPU -5% |
| zen | Gecko | 748 MB | 927 MB | 17.0 | 17 | 0.94 s | mem -1%, CPU -13% |
| firefox | Gecko | 813 MB | 937 MB | 5.0 | 16 | 0.94 s | mem -0%, CPU -4% |
| vivaldi | Blink | 861 MB | 1012 MB | 1.3 | 18 | 0.45 s | mem -0%, CPU -13% |
| gnome-web | WebKit | 999 MB | 1007 MB | 0.1 | 38 | 0.31 s | mem +3%, CPU +0% |
| librewolf | Gecko | 1061 MB | 1418 MB | 2.9 | 15 | 0.94 s | mem +1%, CPU -3% |

### video-vp9

Runner: 4 × AMD EPYC 9V74 80-Core Processor, 15 GB; no earlier run on this CPU to compare with.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 591 MB | 593 MB | 366.7 | 9 | 0.32 s |  |
| palemoon | Goanna | 255 MB | 262 MB | 33.4 | 1 | 0.65 s |  |
| qutebrowser | Blink (Qt WebEngine) | 451 MB | 457 MB | 222.4 | 5 | 0.63 s |  |
| ungoogled-chromium | Blink | 470 MB | 481 MB | 43.5 | 10 | 0.34 s |  |
| falkon | Blink (Qt WebEngine) | 475 MB | 484 MB | 204.3 | 5 | 0.32 s |  |
| chrome | Blink | 479 MB | 542 MB | 41.6 | 13 | 0.32 s |  |
| opera | Blink | 490 MB | 616 MB | 41.1 | 15 | 0.33 s |  |
| brave | Blink | 498 MB | 507 MB | 39.9 | 12 | 0.33 s |  |
| edge | Blink | 532 MB | 646 MB | 41.8 | 12 | 0.44 s |  |
| chromium | Blink | 544 MB | 597 MB | 42.7 | 12 | 0.34 s |  |
| waterfox | Gecko | 691 MB | 1035 MB | 47.1 | 11 | 1.59 s |  |
| floorp | Gecko | 707 MB | 800 MB | 41.2 | 10 | 1.15 s |  |
| vivaldi | Blink | 755 MB | 916 MB | 52.7 | 13 | 0.47 s |  |
| zen | Gecko | 806 MB | 986 MB | 63.0 | 12 | 1.06 s |  |
| firefox | Gecko | 825 MB | 901 MB | 46.6 | 11 | 1.26 s |  |
| librewolf | Gecko | 1179 MB | 1412 MB | 43.8 | 10 | 1.18 s |  |

### video-av1

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261126-0418-ea5b59f-45754226156.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 632 MB | 634 MB | 363.4 | 9 | 0.21 s | mem -2%, CPU -2% |
| palemoon | Goanna | 281 MB | 290 MB | 32.9 | 1 | 0.52 s |  |
| opera | Blink | 484 MB | 617 MB | 38.7 | 15 | 0.34 s | mem -24%, CPU +22% |
| ungoogled-chromium | Blink | 503 MB | 512 MB | 37.2 | 10 | 0.32 s | mem -4%, CPU +18% |
| chrome | Blink | 505 MB | 565 MB | 35.6 | 12 | 0.33 s | mem -5%, CPU +17% |
| brave | Blink | 528 MB | 531 MB | 33.4 | 12 | 0.32 s | mem -4%, CPU +16% |
| qutebrowser | Blink (Qt WebEngine) | 555 MB | 576 MB | 197.0 | 5 | 0.52 s | mem -8%, CPU +15% |
| edge | Blink | 556 MB | 665 MB | 35.5 | 12 | 0.33 s | mem -4%, CPU +16% |
| chromium | Blink | 571 MB | 608 MB | 35.5 | 12 | 0.33 s | mem -3%, CPU +17% |
| falkon | Blink (Qt WebEngine) | 574 MB | 600 MB | 182.9 | 5 | 0.22 s | mem -0%, CPU +11% |
| waterfox | Gecko | 739 MB | 1058 MB | 50.4 | 11 | 1.36 s | mem -2%, CPU +5% |
| floorp | Gecko | 742 MB | 919 MB | 46.9 | 10 | 0.94 s |  |
| vivaldi | Blink | 783 MB | 938 MB | 48.4 | 13 | 0.44 s | mem -3%, CPU +26% |
| zen | Gecko | 848 MB | 989 MB | 66.9 | 12 | 0.95 s | mem -1%, CPU +7% |
| firefox | Gecko | 856 MB | 952 MB | 49.9 | 11 | 0.94 s | mem -1%, CPU +2% |
| librewolf | Gecko | 1395 MB | 1556 MB | 46.0 | 10 | 0.95 s | mem -0%, CPU +8% |

### video-h264

Runner: 4 × INTEL(R) XEON(R) PLATINUM 8573C, 15 GB; no earlier run on this CPU to compare with.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 631 MB | 633 MB | 358.9 | 9 | 0.32 s |  |
| palemoon | Goanna | 276 MB | 280 MB | 44.0 | 1 | 0.63 s |  |
| qutebrowser | Blink (Qt WebEngine) | 482 MB | 494 MB | 264.1 | 5 | 0.63 s |  |
| ungoogled-chromium | Blink | 511 MB | 519 MB | 46.5 | 10 | 0.33 s |  |
| chrome | Blink | 515 MB | 578 MB | 45.6 | 12 | 0.33 s |  |
| falkon | Blink (Qt WebEngine) | 518 MB | 518 MB | 248.2 | 5 | 0.32 s |  |
| brave | Blink | 538 MB | 543 MB | 43.1 | 12 | 0.33 s |  |
| edge | Blink | 560 MB | 672 MB | 45.5 | 12 | 0.44 s |  |
| waterfox | Gecko | 719 MB | 1094 MB | 65.2 | 11 | 1.77 s |  |
| floorp | Gecko | 726 MB | 822 MB | 59.1 | 10 | 1.26 s |  |
| vivaldi | Blink | 792 MB | 953 MB | 59.3 | 13 | 0.56 s |  |
| firefox | Gecko | 831 MB | 903 MB | 67.2 | 11 | 1.25 s |  |
| zen | Gecko | 831 MB | 1014 MB | 90.7 | 12 | 1.25 s |  |
| librewolf | Gecko | 941 MB | 1423 MB | 60.1 | 10 | 1.25 s |  |

### pan

Runner: 4 × AMD EPYC 9V74 80-Core Processor, 15 GB; no earlier run on this CPU to compare with.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 792 MB | 800 MB | 375.2 | 14 | 0.23 s |  |

## Trailboard over time (newest first)

| Run | Trailboard | idle mem / CPU | still mem / CPU | video-vp9 mem / CPU | video-av1 mem / CPU | video-h264 mem / CPU | pan mem / CPU |
|---|---|---|---|---|---|---|---|
| [20261126-1507-e](https://github.com/AyushKr2003/trailboard/actions/runs/45845659696) | `e1edf21` | 574 MB / 4.7 % | 700 MB / 0.9 % | 591 MB / 366.7 % | 632 MB / 363.4 % | 631 MB / 358.9 % | 792 MB / 375.2 % |
| [20261126-0725-b](https://github.com/AyushKr2003/trailboard/actions/runs/45773585181) | `bd53a3d` | 578 MB / 4.7 % | 697 MB / 0.8 % | 619 MB / 366.2 % | 646 MB / 362.8 % | 646 MB / 364.3 % | 749 MB / 361.9 % |
| [20261126-0524-4](https://github.com/AyushKr2003/trailboard/actions/runs/45761289882) | `4c931f8` | 577 MB / 5.1 % | 699 MB / 0.6 % | 622 MB / 365.8 % | 672 MB / 376.2 % | 651 MB / 363.0 % | 746 MB / 368.2 % |
| [20261126-0418-e](https://github.com/AyushKr2003/trailboard/actions/runs/45754226156) | `ea5b59f` | 582 MB / 45.6 % | 697 MB / 0.5 % | 617 MB / 365.2 % | 648 MB / 372.2 % | 643 MB / 365.4 % | 738 MB / 358.0 % |
| [20261125-1734-e](https://github.com/AyushKr2003/trailboard/actions/runs/45665317928) | `ea5b59f` | 570 MB / 37.3 % | 684 MB / 0.6 % | 603 MB / 366.1 % | 640 MB / 364.1 % | 639 MB / 372.5 % | 742 MB / 359.7 % |
