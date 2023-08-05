# Creates 25600.png
# Run usbnoise example, pipe through "xxd -r -p | head -c 25600", run this script
from PIL import Image

import sys

b = open(sys.argv[1], "rb")
b = b.read()

edge = int(len(b)**0.5)
assert edge*edge == len(b), "Image must be square"


def lsb(x):
    # thanks https://stackoverflow.com/questions/5520655/return-index-of-least-significant-bit-in-python
    return (x&-x).bit_length()-1

b = list(lsb(x) for x in b)

print(b[:20])

inp_max = max(b)
print(f"input max is {inp_max}")
range = (50, 200)
scal = (range[1] - range[0] + 1) // inp_max
b = bytes(range[0] + x * scal for x in b)
print(list(b[:20]))

im = Image.frombytes("L", (edge, edge), b)

im.save("im.png")
