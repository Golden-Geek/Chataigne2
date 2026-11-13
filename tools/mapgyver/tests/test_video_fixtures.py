"""Fixture manifests must distinguish decoded HAP blocks from ProRes planes."""
from pathlib import Path
import runpy
import unittest


MODULE = runpy.run_path(str(Path(__file__).parents[1] / "video_fixtures.py"))


class VideoFixtureTests(unittest.TestCase):
    def test_alpha_oracle_rejects_opaque_replacement_and_lost_marker_opacity(self):
        width = 320
        pixels = bytearray(width * 64 * 4)
        sites = [(4, 24, 255), (8, 56, 64), (width - 8, 56, 192)]
        for x, y, alpha in sites:
            pixels[(y * width + x) * 4 + 3] = alpha
        self.assertEqual([x["actual"] for x in MODULE["verify_alpha"](pixels, width)],
                         [255, 64, 192])
        for x, y, _ in sites:
            wrong = pixels.copy()
            wrong[(y * width + x) * 4 + 3] = 0
            with self.assertRaises(ValueError):
                MODULE["verify_alpha"](wrong, width)
        with self.assertRaises(ValueError):
            MODULE["verify_alpha"](pixels[:-1], width)

    def test_stream_contract_rejects_wrong_codec_layout_and_frame_count(self):
        validate = MODULE["validate_stream"]
        for profile in MODULE["PROFILES"].values():
            expected = profile[3]
            info = {**expected, "width": 1920, "height": 1080, "nb_frames": "240"}
            validate(info, expected, 1920, 1080, 240)
            for key in [*expected, "width", "height", "nb_frames"]:
                with self.subTest(codec=profile[0], key=key):
                    wrong = {**info, key: "wrong" if key in expected else 1}
                    with self.assertRaises(ValueError):
                        validate(wrong, expected, 1920, 1080, 240)

    def test_binary_marker_survives_block_aligned_quantization(self):
        for index in [0, 1, 31, 119, 239, 4095]:
            pixels = bytearray(320 * 32 * 3)
            pixels[(24 * 320 + 4) * 3:(24 * 320 + 4) * 3 + 3] = b"\xff" * 3
            for bit in range(12):
                offset = (24 * 320 + 24 + bit * 24) * 3
                pixels[offset:offset + 3] = bytes([235 if index & (1 << bit) else 16]) * 3
            self.assertEqual(MODULE["frame_code"](pixels), index)
        with self.assertRaisesRegex(ValueError, "sentinel"):
            MODULE["frame_code"](bytes(320 * 32 * 3))


if __name__ == "__main__":
    unittest.main()
