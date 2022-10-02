#!/usr/bin/env python

"""
└─ embassy_rp::usb::{impl#5}::start @ /home/matt/3rd/rs/embassy-hack/embassy-rp/src/fmt.rs:112
0.001821 INFO  hello
└─ usb_serial::run @ src/bin/usb_serial.rs:159
0.002251 INFO  initial pulldown del is 41962
└─ usb_serial::run @ src/bin/usb_serial.rs:189
0.015146 INFO  up bbdefd5c down bfdff3e4
└─ usb_serial::run::{closure#3} @ src/bin/usb_serial.rs:359
0.025601 INFO  up e64ed24b down f5792f9d
└─ usb_serial::run::{closure#3} @ src/bin/usb_serial.rs:359
"""

import sys
import csv

def it(f):
    for l in f:
        if not "  up" in l:
            continue

        s = l.split()
        up = int(s[3], 16)
        down = int(s[5], 16)

        yield (up & 0xff, down & 0xff)
        yield (up >> 8 & 0xff, down >> 8 & 0xff)
        yield (up >> 16 & 0xff, down >> 16 & 0xff)
        yield (up >> 24 & 0xff, down >> 24 & 0xff)

w = csv.writer(sys.stdout)
w.writerow(['up', 'down'])
w.writerows(it(sys.stdin))
