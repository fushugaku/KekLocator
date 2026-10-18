#!/usr/bin/env python3
"""Fetch the pinned manufacturer data needed by the translated DSP.

The downloaded firmware is kept outside Git. Hashes pin the exact input used to
produce the static Rust translation; a different firmware is not interchangeable.
"""
import hashlib
import struct
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEST = ROOT / 'crates/engine/firmware'
ROM_HASH = '9cea9f9a998ac7bf05297821290d36ef2843ad72b10f0da583765cc914c58211'
UF2_HASH = 'da9447eb3e4dd978a56b38474ec22fc79bdbc780f5da154f85b19fac04a89304'
BANK_HASH = '1329d0c4fa7323f2d3ba61a4a0112f0264bc7ff25db071ed9fd741c002534982'
BASE_URL = 'https://www.ericasynths.lv/service/file/download/product_id/784/file_id/'


def checked(data, expected):
    actual = hashlib.sha256(data).hexdigest()
    if actual != expected:
        raise RuntimeError(f'Unexpected source checksum: {actual}; expected {expected}')
    return data


def download(file_id):
    request = urllib.request.Request(BASE_URL + str(file_id) + '/', headers={'User-Agent': 'KekLocator-source-setup/1.2'})
    with urllib.request.urlopen(request, timeout=60) as response:
        return response.read(4 * 1024 * 1024)


def decode_uf2(raw):
    if len(raw) % 512:
        raise ValueError('Truncated UF2')
    output, coverage, seen = bytearray(249856), bytearray(249856), set()
    for offset in range(0, len(raw), 512):
        block = raw[offset:offset + 512]
        a, b, flags, address, size, number, count, family = struct.unpack_from('<8I', block)
        if (a, b, struct.unpack_from('<I', block, 508)[0]) != (0x0A324655, 0x9E5D5157, 0x0AB16F30):
            raise ValueError('Invalid UF2 signature')
        start = address - 0x60020000
        if number in seen or count != len(raw) // 512 or not 0 < size <= 476 or not 0 <= start <= len(output) - size or flags & 1:
            raise ValueError('Unexpected UF2 block layout')
        seen.add(number)
        output[start:start + size] = block[32:32 + size]
        coverage[start:start + size] = b'\1' * size
    if not all(coverage):
        raise ValueError('Incomplete firmware image')
    return bytes(output)


def main():
    DEST.mkdir(parents=True, exist_ok=True)
    rom = DEST / 'echolocator-1.05.bin'
    if rom.exists():
        checked(rom.read_bytes(), ROM_HASH)
    else:
        rom.write_bytes(checked(decode_uf2(checked(download(664), UF2_HASH)), ROM_HASH))
    bank = DEST / 'default-backup.elb'
    if bank.exists():
        checked(bank.read_bytes(), BANK_HASH)
    else:
        bank.write_bytes(checked(download(577), BANK_HASH))
    print('Verified Echolocator 1.05 source data and factory bank')


if __name__ == '__main__':
    main()
