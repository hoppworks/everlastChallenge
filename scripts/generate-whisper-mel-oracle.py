"""Generate tiny Whisper goldens with direct CPU DFT; no model or MLX required."""
import ast
import hashlib
import json
import math
from pathlib import Path
import struct
import sys
import zipfile

FILTER_SHA = "7450ae70723a5ef9d341e3cee628c7cb0177f36ce42c44b7ed2bf3325f0f6d4c"
REVISION = "86098128c0b4f24f0e2aa2994de830614b474227"


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def filters(archive, count):
    data = archive.read("mel_%d.npy" % count)
    assert data[:8] == b"\x93NUMPY\x01\x00"
    length = struct.unpack("<H", data[8:10])[0]
    header = ast.literal_eval(data[10:10 + length].decode("ascii"))
    assert header == {"descr": "<f4", "fortran_order": False, "shape": (count, 201)}
    return struct.unpack("<%df" % (count * 201), data[10 + length:])


def oracle(samples, count, padding, bank):
    signal = samples + [0.0] * padding
    window = [0.5 * (1.0 - math.cos(2 * math.pi * i / 400)) for i in range(400)]
    basis = [[complex(math.cos(-2 * math.pi * k * i / 400),
                      math.sin(-2 * math.pi * k * i / 400)) for i in range(400)]
             for k in range(201)]
    spectrum = []
    # Centered reflect padding adds one STFT frame; Whisper drops that last frame.
    for frame in range(len(signal) // 160):
        values = []
        for i in range(400):
            index = frame * 160 + i - 200
            if index < 0:
                index = -index
            if index >= len(signal):
                index = 2 * len(signal) - 2 - index
            values.append(signal[index] * window[i])
        power = [abs(sum(value * phase for value, phase in zip(values, row))) ** 2
                 for row in basis]
        spectrum.extend(math.log10(max(1e-10, sum(power[k] * bank[m * 201 + k]
                                                for k in range(201))))
                        for m in range(count))
    floor = max(spectrum) - 8.0
    return [f32((max(value, floor) + 4.0) / 4.0) for value in spectrum]


def main():
    source, output = map(Path, sys.argv[1:])
    assert hashlib.sha256(source.read_bytes()).hexdigest() == FILTER_SHA
    noise = 7
    tones = []
    for i in range(1600):
        noise = (1664525 * noise + 1013904223) % 2 ** 32
        tones.append(f32(0.4 * math.sin(2 * math.pi * 443.3 * i / 16000)
                         + 0.2 * math.cos(2 * math.pi * 1207.7 * i / 16000)
                         + 0.03 * (noise / 2 ** 31 - 1)))
    impulse = [0.0] * 641
    impulse[0], impulse[320], impulse[640] = 0.75, -0.5, 0.25
    cases = []
    with zipfile.ZipFile(source) as archive:
        for name, samples, count, padding in [
                ("off-bin-tones-and-broadband", tones, 128, 0),
                ("boundary-impulses-with-right-padding", impulse, 80, 320),
                ("silence-floor", [0.0] * 1600, 128, 0)]:
            cases.append({"name": name, "samples": samples, "mels": count,
                          "padding": padding, "shape": [(len(samples) + padding) // 160, count],
                          "expected": oracle(samples, count, padding, filters(archive, count)),
                          "maximumAbsoluteError": 0.0002})
    result = {"schemaVersion": 1, "method": "independent direct DFT, periodic Hann400, hop160, reflect center, final STFT frame removed, exact official Slaney banks, Whisper log clamp",
              "source": "https://github.com/openai/whisper/tree/" + REVISION,
              "filterSha256": FILTER_SHA,
              "generatorSha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "cases": cases}
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("x") as file:
        json.dump(result, file, indent=2, allow_nan=False)
        file.write("\n")


if __name__ == "__main__":
    main()
