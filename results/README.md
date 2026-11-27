# Results

Written by the bench workflow after every run. Chart: <https://ayushkr2003.github.io/trailboard/>

## Bench: Trailboard 5aa154c against 5 browsers

20261127-1348 · 3 runs each, medians · each case on its own runner · [workflow run](https://github.com/AyushKr2003/trailboard/actions/runs/46010069445)

✅ No regression against earlier runs on the same CPU (not checked, no earlier run on their CPU: idle).

Not run, by design (see browsers.toml): gnome-web (video-vp9, video-av1, video-h264).

### idle

Runner: 4 × AMD EPYC 9V74 80-Core Processor, 15 GB; no earlier run on this CPU to compare with.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 591 MB | 591 MB | 2.4 | 9 | 0.22 s |  |
| falkon | Blink (Qt WebEngine) | 412 MB | 413 MB | 9.3 | 5 | 0.21 s |  |
| chrome | Blink | 438 MB | 496 MB | 0.6 | 12 | 0.33 s |  |
| gnome-web | WebKit | 440 MB | 441 MB | 0.0 | 8 | 0.42 s |  |
| brave | Blink | 471 MB | 482 MB | 1.1 | 12 | 0.32 s |  |
| firefox | Gecko | 744 MB | 823 MB | 3.7 | 10 | 1.04 s |  |

### still

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261126-1507-e1edf21-45845659696.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 690 MB | 692 MB | 0.8 | 14 | 0.21 s | mem -1%, CPU -11% |
| falkon | Blink (Qt WebEngine) | 430 MB | 448 MB | 0.3 | 5 | 0.21 s | mem +1%, CPU +0% |
| chrome | Blink | 529 MB | 588 MB | 0.3 | 18 | 0.34 s | mem +0%, CPU +0% |
| brave | Blink | 559 MB | 578 MB | 0.2 | 17 | 0.32 s | mem -0%, CPU +0% |
| firefox | Gecko | 820 MB | 944 MB | 3.4 | 16 | 0.94 s | mem +1%, CPU -32% |
| gnome-web | WebKit | 993 MB | 1022 MB | 0.1 | 38 | 0.31 s | mem -1%, CPU +0% |

### video-vp9

Runner: 4 × AMD EPYC 7763 64-Core Processor, 15 GB; compared with 20261127-1121-879f0c2-45989683312.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 631 MB | 637 MB | 351.5 | 9 | 0.32 s | mem +3%, CPU +1% |
| falkon | Blink (Qt WebEngine) | 480 MB | 488 MB | 254.0 | 5 | 0.32 s | mem +0%, CPU +3% |
| chrome | Blink | 496 MB | 558 MB | 48.5 | 13 | 0.34 s | mem +3%, CPU -5% |
| brave | Blink | 515 MB | 524 MB | 46.2 | 12 | 0.45 s | mem +3%, CPU -3% |
| firefox | Gecko | 820 MB | 877 MB | 54.7 | 11 | 1.26 s | mem +1%, CPU -4% |

### video-av1

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261127-1223-5aa154c-45998021086.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 654 MB | 657 MB | 258.9 | 9 | 0.21 s | mem +4%, CPU +4% |
| chrome | Blink | 528 MB | 586 MB | 34.1 | 12 | 0.22 s | mem +5%, CPU +13% |
| brave | Blink | 552 MB | 555 MB | 33.1 | 12 | 0.32 s | mem +5%, CPU +15% |
| falkon | Blink (Qt WebEngine) | 590 MB | 615 MB | 179.4 | 5 | 0.21 s | mem -2%, CPU +14% |
| firefox | Gecko | 863 MB | 951 MB | 50.3 | 11 | 0.83 s | mem +0%, CPU +9% |

### video-h264

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261127-1223-5aa154c-45998021086.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 644 MB | 651 MB | 256.5 | 9 | 0.21 s | mem +0%, CPU +3% |
| falkon | Blink (Qt WebEngine) | 507 MB | 513 MB | 181.3 | 5 | 0.21 s | mem +0%, CPU +8% |
| chrome | Blink | 517 MB | 579 MB | 31.6 | 12 | 0.22 s | mem -0%, CPU +4% |
| brave | Blink | 537 MB | 542 MB | 31.3 | 12 | 0.32 s | mem +0%, CPU +6% |
| firefox | Gecko | 842 MB | 921 MB | 47.0 | 11 | 0.83 s | mem +0%, CPU +1% |

### pan

Runner: 4 × Intel(R) Xeon(R) 6975P-C, 15 GB; compared with 20261127-1223-5aa154c-45998021086.

| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change (same CPU) |
|---|---|---|---|---|---|---|---|
| **trailboard** | Blink (CEF) | 814 MB | 820 MB | 348.0 | 14 | 0.22 s | mem +2%, CPU -1% |

## Trailboard over time (newest first)

| Run | Trailboard | idle mem / CPU | still mem / CPU | video-vp9 mem / CPU | video-av1 mem / CPU | video-h264 mem / CPU | pan mem / CPU |
|---|---|---|---|---|---|---|---|
| [20261127-1348-5](https://github.com/AyushKr2003/trailboard/actions/runs/46010069445) | `5aa154c` | 591 MB / 2.4 % | 690 MB / 0.8 % | 631 MB / 351.5 % | 654 MB / 258.9 % | 644 MB / 256.5 % | 814 MB / 348.0 % |
| [20261127-1223-5](https://github.com/AyushKr2003/trailboard/actions/runs/45998021086) | `5aa154c` | 579 MB / 3.7 % | 680 MB / 0.8 % | 623 MB / 269.9 % | 629 MB / 248.1 % | 642 MB / 248.5 % | 801 MB / 350.2 % |
| [20261127-1121-8](https://github.com/AyushKr2003/trailboard/actions/runs/45989683312) | `879f0c2` | 585 MB / 3.2 % | 704 MB / 0.7 % | 611 MB / 347.8 % | 646 MB / 332.3 % | 645 MB / 272.2 % | 812 MB / 352.6 % |
| [20261126-1507-e](https://github.com/AyushKr2003/trailboard/actions/runs/45845659696) | `e1edf21` | 574 MB / 4.7 % | 700 MB / 0.9 % | 591 MB / 366.7 % | 632 MB / 363.4 % | 631 MB / 358.9 % | 792 MB / 375.2 % |
| [20261126-0725-b](https://github.com/AyushKr2003/trailboard/actions/runs/45773585181) | `bd53a3d` | 578 MB / 4.7 % | 697 MB / 0.8 % | 619 MB / 366.2 % | 646 MB / 362.8 % | 646 MB / 364.3 % | 749 MB / 361.9 % |
| [20261126-0524-4](https://github.com/AyushKr2003/trailboard/actions/runs/45761289882) | `4c931f8` | 577 MB / 5.1 % | 699 MB / 0.6 % | 622 MB / 365.8 % | 672 MB / 376.2 % | 651 MB / 363.0 % | 746 MB / 368.2 % |
| [20261126-0418-e](https://github.com/AyushKr2003/trailboard/actions/runs/45754226156) | `ea5b59f` | 582 MB / 45.6 % | 697 MB / 0.5 % | 617 MB / 365.2 % | 648 MB / 372.2 % | 643 MB / 365.4 % | 738 MB / 358.0 % |
| [20261125-1734-e](https://github.com/AyushKr2003/trailboard/actions/runs/45665317928) | `ea5b59f` | 570 MB / 37.3 % | 684 MB / 0.6 % | 603 MB / 366.1 % | 640 MB / 364.1 % | 639 MB / 372.5 % | 742 MB / 359.7 % |
