#!/usr/bin/env python3

import struct
import json
import os
from itertools import zip_longest
from typing import Tuple
import argparse

from PIL import Image

def group(seq, n, fillvalue=None):
    return zip_longest(*[iter(seq)] * n, fillvalue=fillvalue)

def byte_from_bits(bits):
    return ((bits[0] & 1) << 7
            | (bits[1] & 1) << 6
            | (bits[2] & 1) << 5
            | (bits[3] & 1) << 4
            | (bits[4] & 1) << 3
            | (bits[5] & 1) << 2
            | (bits[6] & 1) << 1
            | (bits[7]) & 1)

def to_rgb565_and_mask(rgba: Tuple[int, int, int, int]) -> Tuple[int, bool]:
    r, g, b, a = rgba
    return (((r >> 3) & 0b00011111) << 11
                | ((g >> 2) & 0b00111111) << 5
                | (b >> 3) & 0b00011111,
            a != 0)

def convert_to_rgb565(input_file: str,
                      output_rgb_file: str,
                      output_mask_file: str,
                      print_size_json: bool = False):
    img = Image.open(input_file)
    rgb = []
    mask = []
    if print_size_json:
        print(json.dumps({'width': img.width, 'height': img.height}))
    else:
        print(f'{input_file} size: {img.size}')
    for row in range(img.height):
        for col in range(img.width):
            pixel_rgb, pixel_mask = to_rgb565_and_mask(img.getpixel((col, row)))
            rgb.append(pixel_rgb)
            mask.append(pixel_mask)
    os.makedirs(os.path.dirname(output_rgb_file), exist_ok=True)
    os.makedirs(os.path.dirname(output_mask_file), exist_ok=True)
    with open(output_rgb_file, 'wb') as f:
        u16_rgb = [struct.pack('>H', pixel) for pixel in rgb]
        f.write(b''.join(u16_rgb))
    with open(output_mask_file, 'wb') as f:
        u8_mask = [bytes([byte_from_bits(g)]) for row in group(mask, img.width) for g in group(row, 8, fillvalue=False)]
        f.write(b''.join(u8_mask))

parser = argparse.ArgumentParser('convert image to RGB565 + transparency mask files')
parser.add_argument('-i', '--input', type=str, required=True, help='Image to convert')
parser.add_argument('-c', '--output-color', type=str, required=True, help='Path to save RGB565 image data to')
parser.add_argument('-m', '--output-mask', type=str, required=True, help='Path to save transparency mask data to')
parser.add_argument('-p', '--print-size-json', action='store_true', help='Print image size in JSON format')
args = parser.parse_args()

convert_to_rgb565(input_file=args.input,
                  output_rgb_file=args.output_color,
                  output_mask_file=args.output_mask,
                  print_size_json=args.print_size_json)
