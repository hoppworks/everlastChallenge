import importlib.util
from pathlib import Path
import struct
import hashlib
import io
import tarfile
import tempfile
from unittest import mock
import json
from contextlib import redirect_stdout
import unittest

SPEC = importlib.util.spec_from_file_location("corpus", Path(__file__).resolve().parents[2] / "scripts" / "corpus.py")
corpus = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(corpus)

class CorpusTests(unittest.TestCase):
    @staticmethod
    def tsv(identifier=1, filename="1.wav", reference='He said "quoted words".', frames=64000):
        return ("{}\t{}\t{}\tnormalized\tcharacters\t{}\t0\n".format(identifier, filename, reference, frames)).encode()

    @staticmethod
    def wav(values):
        payload = b"".join(struct.pack("<f", value) for value in values)
        fmt = struct.pack("<HHIIHH", 3, 1, 16000, 64000, 4, 32)
        body = b"WAVEfmt " + struct.pack("<I", len(fmt)) + fmt + b"data" + struct.pack("<I", len(payload)) + payload
        return b"RIFF" + struct.pack("<I", len(body)) + body

    def test_selection_is_sorted_unique_and_central_padding_preserves_the_selected_recording(self):
        rows = []
        for identifier, filename, seconds in [(4, "4.wav", 14.5), (1, "2.wav", 7), (1, "1.wav", 4.5), (2, "3.wav", 9.5), (0, "0.wav", 3), (5, "5.wav", 31)]:
            rows.extend(corpus.parse_tsv(self.tsv(identifier, filename, frames=int(seconds * 16000))))
        selected = corpus.select_cases(rows, 2, central=True)
        self.assertEqual([row["sentenceId"] for row in selected], [1, 2, 4])
        self.assertEqual(selected[0]["filename"], "1.wav")
        self.assertEqual([row["targetFrames"] for row in selected], [80000, 160000, 240000])
        self.assertEqual(selected[0]["frames"], 72000)

    def test_float32_quantization_is_nearest_even_saturating_and_frame_checked(self):
        values = [-2, -1, -1.5 / 32768, -0.5 / 32768, 0.5 / 32768, 1.5 / 32768, 1, 2]
        pcm = corpus.decode_f32_wav(self.wav(values), len(values))
        self.assertEqual(list(struct.unpack("<8h", pcm)), [-32768, -32768, -2, 0, 0, 2, 32767, 32767])
        with self.assertRaises(corpus.CorpusError):
            corpus.decode_f32_wav(self.wav(values), len(values) + 1)
        for value in [float("inf"), float("nan")]:
            with self.assertRaises(corpus.CorpusError):
                corpus.decode_f32_wav(self.wav([value]), 1)

    @staticmethod
    def read_fixture(path, split, names):
        raw = path.read_bytes()
        with corpus.open_verified(path, len(raw), hashlib.sha256(raw).hexdigest()) as source:
            return corpus.read_selected(source, split, names)

    @staticmethod
    def archive(path, entries):
        with tarfile.open(path, "w:gz") as target:
            for name, data, kind in entries:
                member = tarfile.TarInfo(name)
                member.type = kind
                member.size = len(data) if kind == tarfile.REGTYPE else 0
                if kind == tarfile.SYMTYPE:
                    member.linkname = "../private"
                target.addfile(member, io.BytesIO(data) if member.size else None)

    def test_verified_archive_selects_only_exact_regular_paths_and_rejects_duplicates(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "source.tar.gz"
            wav = self.wav([0.25, -0.5])
            self.archive(path, [("test/1.wav", wav, tarfile.REGTYPE), ("other/2.wav", b"ignore", tarfile.REGTYPE)])
            raw = path.read_bytes()
            corpus.verify_file(path, len(raw), hashlib.sha256(raw).hexdigest())
            self.assertEqual(self.read_fixture(path, "test", {"1.wav"}), {"1.wav": wav})
            for size, digest in [(len(raw) + 1, hashlib.sha256(raw).hexdigest()), (len(raw), "0" * 64)]:
                with self.assertRaises(corpus.CorpusError):
                    corpus.verify_file(path, size, digest)
            for entries in [[("../test/1.wav", wav, tarfile.REGTYPE)], [("test/1.wav", wav, tarfile.REGTYPE)] * 2, [("test/1.wav", b"", tarfile.SYMTYPE)]]:
                self.archive(path, entries)
                with self.assertRaises(corpus.CorpusError):
                    self.read_fixture(path, "test", {"1.wav"})

    def test_padding_quiet_and_constructed_noise_are_explicit_and_do_not_cut_speech(self):
        pcm = struct.pack("<4h", 100, -100, 200, -200)
        padded = corpus.pad_pcm(pcm, 6)
        self.assertEqual(padded, pcm + b"\0" * 4)
        with self.assertRaises(corpus.CorpusError):
            corpus.pad_pcm(pcm, 3)
        self.assertEqual(struct.unpack("<4h", corpus.scale_pcm(pcm, 0.05)), (5, -5, 10, -10))
        noise = corpus.generated_pcm("noise", 640)
        self.assertEqual(noise, corpus.generated_pcm("noise", 640))
        self.assertNotEqual(noise, b"\0" * len(noise))
        stress, minimum = corpus.construct_masked([pcm], 640)
        self.assertEqual(len(stress), 1280)
        self.assertGreaterEqual(minimum, 0.005)
        with mock.patch.object(corpus, "_noise", return_value=iter([0] * 640)):
            with self.assertRaises(corpus.CorpusError):
                corpus.construct_masked([b"\0" * 640], 640)

    def test_tiny_pinned_builder_is_frozen_reproducible_and_records_source_hashes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            source.mkdir()
            pins = {}
            for locale, language in [("de_de", "de"), ("en_us", "en")]:
                for split in ["dev", "test"]:
                    key = locale + "-" + split
                    entries, metadata = [], b""
                    for identifier, frames in [(1, 72000), (2, 152000), (3, 232000)]:
                        if split == "dev":
                            identifier += 100
                        filename = str(identifier) + ".wav"
                        data = self.wav([0.1] * frames)
                        entries.append((split + "/" + filename, data, tarfile.REGTYPE))
                        metadata += self.tsv(identifier, filename, "Independent words {}".format(identifier), frames)
                    archive = source / (key + ".tar.gz")
                    self.archive(archive, entries)
                    (source / (key + ".tsv")).write_bytes(metadata)
                    raw = archive.read_bytes()
                    pins[key] = {"language": language, "split": split, "archiveBytes": len(raw), "archiveSha256": hashlib.sha256(raw).hexdigest(), "tsvBytes": len(metadata), "tsvSha256": hashlib.sha256(metadata).hexdigest()}
            manifest = root / "frozen.json"
            with mock.patch.object(corpus, "ROOT", root), mock.patch.object(corpus, "PINS", pins), mock.patch.object(corpus, "BASE_COUNTS", {"de": 2, "en": 2}), mock.patch.object(corpus, "CALIBRATION_COUNT", 1), mock.patch.object(corpus, "QUIET_COUNT", 1), mock.patch.object(corpus, "MAX_FRAMES", 640000):
                report = corpus.build(source, manifest)
                before = manifest.read_bytes()
                self.assertEqual(report, corpus.build(source, manifest))
                self.assertEqual(before, manifest.read_bytes())
                terminal = io.StringIO()
                with redirect_stdout(terminal):
                    self.assertEqual(corpus.main(["--source", str(source), "--manifest", str(manifest)]), 0)
                self.assertIn("coverage remains explicitly partial", terminal.getvalue())
                self.assertEqual(manifest.stat().st_mode & 0o777, 0o600)
                self.assertEqual(len([case for case in report["cases"] if "central" in case["tags"]]), 6)
                for case in report["cases"]:
                    audio = root / case["audio"]["path"]
                    self.assertEqual(hashlib.sha256(audio.read_bytes()).hexdigest(), case["audio"]["sha256"])
                    self.assertEqual(audio.stat().st_mode & 0o777, 0o600)
                quiet = [case for case in report["cases"] if "quiet" in case["tags"]]
                self.assertEqual(len(quiet), 4)
                self.assertEqual({case["split"] for case in quiet}, {"calibration", "acceptance"})
                stress = next(case for case in report["cases"] if "max-duration" in case["tags"])
                self.assertEqual(stress["audio"]["frames"], 640000)
                self.assertGreaterEqual(stress["source"]["transformations"][0]["measuredMinimumWindowRms"], 0.005)
                self.assertEqual(stress["source"]["clips"][0]["sourceStartFrame"], 0)
                damaged = root / report["cases"][0]["audio"]["path"]
                original = damaged.read_bytes()
                damaged.write_bytes(b"foreign change")
                with self.assertRaises(corpus.CorpusError):
                    corpus.build(source, manifest)
                self.assertEqual(damaged.read_bytes(), b"foreign change")
                self.assertEqual(manifest.read_bytes(), before)
                damaged.write_bytes(original)
                manifest.write_bytes(b"frozen different")
                with self.assertRaises(corpus.CorpusError):
                    corpus.build(source, manifest)

    def test_metadata_duplicates_paths_bounds_and_malformed_wav_are_explicit_errors(self):
        for invalid in [self.tsv() * 2, self.tsv(filename="../1.wav"), self.tsv(frames=4800001), b"wrong\tcolumns\n", b"x" * (corpus.MAX_TSV_BYTES + 1)]:
            with self.assertRaises(corpus.CorpusError):
                corpus.parse_tsv(invalid)
        valid = self.wav([0.25])
        for invalid in [valid[:-1], valid + b"extra", valid.replace(struct.pack("<I", 16000), struct.pack("<I", 8000), 1), b"invalid"]:
            with self.assertRaises(corpus.CorpusError):
                corpus.decode_f32_wav(invalid, 1)
        with self.assertRaises(corpus.CorpusError):
            corpus.select_cases(corpus.parse_tsv(self.tsv()), 2)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "oversized.tar.gz"
            self.archive(path, [("test/1.wav", valid, tarfile.REGTYPE)])
            with mock.patch.object(corpus, "MAX_MEMBER_BYTES", len(valid) - 1):
                with self.assertRaises(corpus.CorpusError):
                    self.read_fixture(path, "test", {"1.wav"})
            with self.assertRaises(corpus.CorpusError):
                self.read_fixture(path, "test", {"2.wav"})

    def test_archive_replacement_between_verification_and_extraction_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "source.tar.gz"
            replacement = Path(directory) / "replacement.tar.gz"
            self.archive(path, [("test/1.wav", self.wav([0.25]), tarfile.REGTYPE)])
            self.archive(replacement, [("test/1.wav", self.wav([0.75]), tarfile.REGTYPE)])
            original = path.read_bytes()
            with corpus.open_verified(path, len(original), hashlib.sha256(original).hexdigest()) as verified:
                replacement.replace(path)
                with self.assertRaises(corpus.CorpusError):
                    corpus.read_selected(verified, "test", {"1.wav"})

    def test_in_place_archive_change_is_not_trusted_by_the_extraction_reader(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "source.tar.gz"
            replacement = Path(directory) / "replacement.tar.gz"
            self.archive(path, [("test/1.wav", self.wav([0.25]), tarfile.REGTYPE)])
            self.archive(replacement, [("test/1.wav", self.wav([0.75]), tarfile.REGTYPE)])
            original = path.read_bytes()
            with corpus.open_verified(path, len(original), hashlib.sha256(original).hexdigest()) as verified:
                path.write_bytes(replacement.read_bytes())
                with self.assertRaises(corpus.CorpusError):
                    corpus.read_selected(verified, "test", {"1.wav"})

    def test_tsv_uses_literal_tabs_and_raw_reference_and_allows_distinct_recordings_of_one_sentence(self):
        rows = corpus.parse_tsv(self.tsv(2, "2.wav") + self.tsv(2, "3.wav"))
        self.assertEqual(len(rows), 2)
        self.assertEqual(rows[0]["reference"], 'He said "quoted words".')
        self.assertEqual(rows[0]["sentenceId"], 2)
        self.assertEqual(rows[1]["filename"], "3.wav")

if __name__ == "__main__":
    unittest.main()
