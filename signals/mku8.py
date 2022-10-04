#!/usr/bin/env python

import struct
import sys

fn = sys.argv[1]

s = bytes(int(l.strip()) % 256 for l in open(fn))
open(f'{fn}.bin8', 'wb').write(s)
