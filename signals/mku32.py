#!/usr/bin/env python

import struct
import sys

fn = sys.argv[1]

s = b''.join(struct.pack('>i', int(l.strip())) for l in open(fn))
open(f'{fn}.bin32', 'wb').write(s)
