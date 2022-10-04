#!/usr/bin/env python

import struct
import sys

fn = sys.argv[1]

s = b''.join(struct.pack('>h', int(l.strip())) for l in open(fn))
open(f'{fn}.bin', 'wb').write(s)
