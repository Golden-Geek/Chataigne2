from pathlib import Path
import runpy
import unittest

COLLECTOR = runpy.run_path(str(Path(__file__).parents[1] / "collect-evidence.py"))


class CollectorTests(unittest.TestCase):
    def test_stream_larger_than_terminal_limit_preserves_every_byte_and_native_failure(self):
        payload = (b'{"payload":"a\\n\\\"b"}\n' * 100_000) + b'last byte\n'
        self.assertGreater(len(payload), 1_048_576)
        items = list(COLLECTOR["records"](payload, 1))
        self.assertTrue(all(len(item["text"]) <= 65_536 for item in items[:-1]))
        self.assertEqual(COLLECTOR["reconstruct"](items), (payload, 1))

    def test_incomplete_reordered_and_changed_chunks_do_not_decode(self):
        items = list(COLLECTOR["records"](b'x' * 140_000, 0))
        mutations = [items[:-1], [items[1], items[0], *items[2:]], [*items[1:]],
                     [dict(items[0], text='changed'), *items[1:]]]
        for mutation in mutations:
            with self.assertRaises(ValueError):
                COLLECTOR["reconstruct"](mutation)

    def test_empty_failed_runner_output_keeps_its_terminal_status(self):
        self.assertEqual(COLLECTOR["reconstruct"](list(COLLECTOR["records"](b'', 2))), (b'', 2))


if __name__ == "__main__":
    unittest.main()
